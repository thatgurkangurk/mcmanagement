# mcmanager

VERY experimental web console for [itzg/minecraft-server](https://docker-minecraft-server.readthedocs.io/en/latest/) servers

## usage


> [!CAUTION]
> this tool does not (and will not) provide any way to authenticate users !
>
> you should instead use a reverse proxy like [Traefik](https://traefik.io/)

docker compose usage example:

```yaml
services:
  mcmanagement:
    image: ghcr.io/thatgurkangurk/mcmanagement:latest # or pin a specific version. want the latest commit? use `we-test-in-production` (not recommended, might be unstable !)
    ports:
      - 8080:8080
    volumes:
      - ./data/servers.json:/app/servers.json:ro
```

create a `./data/servers.json` file with the following content

```json
{
  "servers": [
    { "id": "<server id>", "name": "<server name>", "url": "ws://<your ip>:<port>/console", "password": "<rcon password>" }
  ]
}
```

make sure to read [the guide from itzg on how to enable WebSocket console on your server](https://docker-minecraft-server.readthedocs.io/en/latest/sending-commands/websocket/) !