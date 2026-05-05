FROM python:3.12-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev

COPY main.py ./
COPY taptime/ ./taptime/

ENV DB_PATH=/data/taptime.db

VOLUME ["/data"]
EXPOSE 8080

CMD ["uv", "run", "python", "main.py"]
