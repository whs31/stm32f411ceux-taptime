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
}
