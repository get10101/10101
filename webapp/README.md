# DLC Connect

<center>
    <img src="https://raw.githubusercontent.com/get10101/10101/main/logos/1000x1000.svg" alt="drawing" width="200"/>
</center>

## Local development

For local development there are two parts: 1. the backend and 2. the frontend. You need to start both.

### Run backend only

```bash
just run-web-backend
```

### Run frontend only

```bash
just run-web
```

### Release build

```bash
just build-web-release
```

### Run the Rust app

#### With TLS

```bash
cargo run --bin webapp -- --data-dir ../data --secure
```

The web interface will be reachable under `https://localhost:3001`.

#### Without TLS

```bash
cargo run -- --data-dir ../data
```

The web interface will be reachable under `http://localhost:3001`

### Troubleshooting

If you can't see anything, you probably forgot to run `just build-web-release` before.

## Production

To start DLC-Connect for a productive build we recommend setting up a docker-compose file which includes a docker [Watchtower](https://github.com/containrrr/watchtower).
The purpose of this Watchtower is to keep DLC Connect up to date.

### Docker-compose file

Go into a directory of your choice and create a `docker-compose.yaml`.

The content should look like this:

```yaml
version: "3.8"
services:
  webapp:
    image: ghcr.io/get10101/10101/webapp:release
    user: 1000:1000
    container_name: webapp
    command: |
      --coordinator-endpoint=022ae8dbec1caa4dac93f07f2ebf5ad7a5dd08d375b79f11095e81b065c2155156@66.248.204.223:9045
      --esplora=http://api.10101.finance:3000
      --password=super_secret_password_please_change_before_use
      --coordinator-http-port=80
      --cert-dir=webapp/certs
      --secure
      mainnet
    ports:
      - "3001:3001"
    volumes:
      - ./data:/data
    restart: unless-stopped
  watchtower:
    image: containrrr/watchtower
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    command: --interval 30
```

> ⚠️ **Make sure to edit the file above and change the password.**

Create a directory called `data`.
This directory will include your database and channel data.

> **⚠️ Don't delete it or you risk losing funds!**

```bash
mkdir data
```

Afterward you can run the application using:

```bash
docker-compose up -d
```

You will find the application running on `https://localhost:3001`.

Note: you might need to open a port on your machine to be able to access it via the internet.

## API

Dlc connect comes with its own Swagger/OpenApi UI and Redoc UI. You can find it under:

- Swagger: https://localhost:3001/swagger-ui/
- Redoc: https://localhost:3001/redoc

### How to interact with the backend with `curl`

We need to store cookies between `curl` calls. For that you can use the `curl`'s cookie jar:

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt \
  -X POST http://localhost:3001/api/login \
  -d '{ "password": "satoshi" }' -H "Content-Type: application/json" -v
```

This will read and store the cookies in `.cookie-jar.txt`. So on the next call you can reference it the same way:

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt http://localhost:3001/api/balance
```
