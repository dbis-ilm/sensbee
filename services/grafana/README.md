## Obersability for SensBee

This is an optional Observability Stack.

The SensBee & event_handling binaries are instrumented to emit logs and traces to the otel-collector which is part of the base compose file.
By default that otel-collector bundles these logs&traces and forwards them to this stack.

Simply use

```
docker compose up -d
```

inside this folder to bring it up.

Point your browser to

```
http://localhost:3000
```

to interact with the grafana Dashboard.

## NOTE

Make sure that the SensBee stack has already started because this stack depends on the network created by it.
