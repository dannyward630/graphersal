#!/usr/bin/env python3
"""Serve the playground for local development without browser caching.

`python3 -m http.server` sends no cache headers, so browsers keep old ES modules and the engine
worker after a rebuild. This is the same static server with `Cache-Control: no-store` on every
answer, served by threads with a listen backlog of 128: the page loads its ES modules in one burst of
parallel requests, and the default backlog of 5 makes the system reset the connections beyond it
(`ERR_CONNECTION_RESET`, the page never starts). Usage (from the repository root):

    python3 playground/serve.py            # http://localhost:8000/
    python3 playground/serve.py 8080
"""
import functools
import http.server
import os
import sys


class NoStoreHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


class BurstServer(http.server.ThreadingHTTPServer):
    # The listen backlog (`socketserver` default: 5). The OS caps it (macOS/Linux: 128 or more).
    request_queue_size = 128
    daemon_threads = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    root = os.path.dirname(os.path.abspath(__file__))
    handler = functools.partial(NoStoreHandler, directory=root)
    with BurstServer(("127.0.0.1", port), handler) as server:
        print(f"Serving the playground on http://localhost:{port}/ (no browser caching)")
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
