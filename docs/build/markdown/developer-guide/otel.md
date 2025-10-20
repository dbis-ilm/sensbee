<a id="opentelemetry"></a>

# OpenTelementry

All Rust-based services within this project are instrumented to emit **traces and logs**. The compose stack includes a collector service that forwards them to Grafana.

**NOTE:** The SensBee stack must be already running! This is because the shared Network will be created by the main stack.

The Grafana stack is optional and can be started with:

```sh
cd services/grafana && docker compose up -d
```

Then, point your browser to:

```default
http://localhost:3000/
```

#### HINT
The Grafana stack is already configured to allow for querying traces and displaying logs.
