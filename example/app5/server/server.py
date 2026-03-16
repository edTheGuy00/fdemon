#!/usr/bin/env python3
"""
Slow-starting backend server for testing fdemon's pre-app custom sources.

Simulates a backend that takes ~5 seconds to initialize before it can
serve requests. fdemon's ready_check will poll /health until it gets a 200,
then launch the Flutter app.
"""

import json
import time
from http.server import HTTPServer, BaseHTTPRequestHandler

PORT = 8085

# In-memory store
todos = [
    {"id": 1, "title": "Buy groceries", "done": False},
    {"id": 2, "title": "Walk the dog", "done": True},
]
next_id = 3


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            self._json_response(200, {"status": "ok"})
        elif self.path == "/api/todos":
            self._json_response(200, todos)
        else:
            self._json_response(404, {"error": "not found"})

    def do_POST(self):
        global next_id
        if self.path == "/api/todos":
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length)) if length else {}
            todo = {"id": next_id, "title": body.get("title", ""), "done": False}
            next_id += 1
            todos.append(todo)
            self._json_response(201, todo)
        else:
            self._json_response(404, {"error": "not found"})

    def _json_response(self, code, data):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        # Log to stdout so fdemon captures it
        print(f"[server] {fmt % args}", flush=True)


def main():
    print("[server] Starting up... (simulating slow init)", flush=True)

    # Simulate slow startup: loading config, connecting to DB, warming caches
    for i in range(5):
        time.sleep(1)
        print(f"[server] Initializing... ({i + 1}/5)", flush=True)

    print(f"[server] Ready — listening on http://localhost:{PORT}", flush=True)

    server = HTTPServer(("127.0.0.1", PORT), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[server] Shutting down", flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
