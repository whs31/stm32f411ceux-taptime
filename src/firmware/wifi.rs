use embassy_stm32::{
  mode::Async,
  usart::{UartRx, UartTx},
};
use embassy_time::{with_timeout, Duration, Timer};

pub struct Wifi {
  tx: UartTx<'static, Async>,
  rx: UartRx<'static, Async>,
}

impl Wifi {
  pub fn new(tx: UartTx<'static, Async>, rx: UartRx<'static, Async>) -> Self {
    Self { tx, rx }
  }

  /// Send a command and wait for expected response, with timeout
  async fn cmd(&mut self, cmd: &[u8], expected: &[u8], timeout_ms: u64) -> bool {
    self.tx.write(cmd).await.unwrap();
    let mut buf = [0u8; 128];
    match with_timeout(
      Duration::from_millis(timeout_ms),
      self.rx.read_until_idle(&mut buf),
    )
    .await
    {
      Ok(Ok(n)) => {
        let got = &buf[..n];
        defmt::debug!("ESP << {:a}", got);
        got.windows(expected.len()).any(|w| w == expected)
      }
      _ => {
        defmt::warn!("ESP timeout/error waiting for {:a}", expected);
        false
      }
    }
  }

  pub async fn connect(&mut self, ssid: &str, password: &str) -> bool {
    // Disable echo
    self.cmd(b"ATE0\r\n", b"OK", 1000).await;

    // Station mode
    if !self.cmd(b"AT+CWMODE=1\r\n", b"OK", 3000).await {
      defmt::error!("Failed to set station mode");
      return false;
    }

    // Join AP
    let join_cmd = alloc::format!(
      "AT+CWJAP=\"{}\",\"{}\"\r\n",
      ssid
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(',', "\\,"),
      password
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(',', "\\,"),
    );
    let mut buf = [0u8; 128];
    self.tx.write(join_cmd.as_bytes()).await.unwrap();

    // CWJAP can take up to 15s and sends multiple lines — drain until OK or FAIL
    let deadline = embassy_time::Instant::now() + Duration::from_millis(15000);
    loop {
      match with_timeout(
        Duration::from_millis(3000),
        self.rx.read_until_idle(&mut buf),
      )
      .await
      {
        Ok(Ok(n)) => {
          let got = &buf[..n];
          defmt::info!("ESP << {:a}", got);
          if got.windows(2).any(|w| w == b"OK") {
            break;
          }
          if got.windows(4).any(|w| w == b"FAIL") || got.windows(16).any(|w| w == b"ERROR") {
            defmt::error!("CWJAP rejected: {:a}", got);
            return false;
          }
        }
        _ => {
          defmt::warn!("ESP read timeout during CWJAP");
        }
      }
      if embassy_time::Instant::now() >= deadline {
        defmt::error!("CWJAP timed out");
        return false;
      }
    }

    Timer::after(Duration::from_millis(500)).await;
    // Confirm IP assigned
    if !self.cmd(b"AT+CIFSR\r\n", b"STAIP", 5000).await {
      defmt::error!("No IP assigned");
      return false;
    }

    defmt::info!("WiFi connected");
    self.cmd(b"AT+GMR\r\n", b"OK", 2000).await;
    true
  }

  /// POST JSON to `host:port/path` and return the response body, or None on failure.
  pub async fn http_post(
    &mut self,
    host: &str,
    port: u16,
    path: &str,
    body: &str,
    secret: &str,
  ) -> Option<alloc::string::String> {
    // 1. Open TCP connection
    let connect_cmd = alloc::format!("AT+CIPSTART=\"TCP\",\"{}\",{}\r\n", host, port);
    self.tx.write(connect_cmd.as_bytes()).await.unwrap();

    let mut buf = [0u8; 256];
    let connected = match with_timeout(
      Duration::from_millis(5000),
      self.rx.read_until_idle(&mut buf),
    )
    .await
    {
      Ok(Ok(n)) => {
        let got = &buf[..n];
        defmt::debug!("ESP << {:a}", got);
        got.windows(7).any(|w| w == b"CONNECT") && !got.windows(5).any(|w| w == b"ERROR")
      }
      _ => false,
    };

    if !connected {
      defmt::warn!("TCP connect failed");
      return None;
    }

    // 2. Build HTTP request
    let extra_headers = if !secret.is_empty() {
      alloc::format!("X-Secret: {}\r\n", secret)
    } else {
      alloc::string::String::new()
    };
    let request = alloc::format!(
      "POST {} HTTP/1.0\r\nHost: {}:{}\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\n\r\n{}",
      path, host, port, extra_headers, body.len(), body
    );

    // 3. Request data send slot
    let cipsend = alloc::format!("AT+CIPSEND={}\r\n", request.len());
    self.tx.write(cipsend.as_bytes()).await.unwrap();

    let got_prompt = match with_timeout(
      Duration::from_millis(3000),
      self.rx.read_until_idle(&mut buf),
    )
    .await
    {
      Ok(Ok(n)) => buf[..n].contains(&b'>'),
      _ => false,
    };

    if !got_prompt {
      defmt::warn!("No CIPSEND prompt");
      self.cmd(b"AT+CIPCLOSE\r\n", b"OK", 1000).await;
      return None;
    }

    // 4. Send HTTP request
    self.tx.write(request.as_bytes()).await.unwrap();

    // 5. Accumulate response until TCP closes ("CLOSED") or timeout
    let mut resp_buf = [0u8; 512];
    let mut total = 0usize;
    let deadline = embassy_time::Instant::now() + Duration::from_millis(5000);

    loop {
      let slice = &mut resp_buf[total..];
      if slice.is_empty() {
        break;
      }
      match with_timeout(Duration::from_millis(1500), self.rx.read_until_idle(slice)).await {
        Ok(Ok(n)) if n > 0 => {
          defmt::debug!("HTTP chunk ({} B): {:a}", n, &resp_buf[total..total + n]);
          total += n;
          if resp_buf[..total].windows(6).any(|w| w == b"CLOSED") {
            break;
          }
        }
        _ => break,
      }
      if embassy_time::Instant::now() >= deadline {
        break;
      }
    }

    let _ = self.cmd(b"AT+CIPCLOSE\r\n", b"OK", 1000).await;

    // 6. Extract JSON body:
    //    Buffer looks like: "...SEND OK\r\n\r\n+IPD,222:HTTP/1.0 200 OK\r\n...headers...\r\n\r\n{json}CLOSED\r\n"
    //    Skip past "+IPD,<len>:" to reach the raw HTTP response, then find \r\n\r\n.
    let data = &resp_buf[..total];
    let ipd_pos = data.windows(5).position(|w| w == b"+IPD,")?;
    let colon_off = data[ipd_pos..].iter().position(|&b| b == b':')?;
    let http_start = ipd_pos + colon_off + 1;
    let body_start = data[http_start..]
      .windows(4)
      .position(|w| w == b"\r\n\r\n")
      .map(|p| http_start + p + 4)?;
    let body_bytes = &data[body_start..];
    let json_end = body_bytes.iter().rposition(|&b| b == b'}').map(|p| p + 1)?;

    let body_str = core::str::from_utf8(&body_bytes[..json_end]).ok()?;
    defmt::info!("HTTP body: {:a}", body_str.as_bytes());
    Some(alloc::string::String::from(body_str))
  }
}
