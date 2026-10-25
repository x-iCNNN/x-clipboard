"""Loopback-only WsgiDAV fixture. pip install WsgiDAV cheroot before use."""
import argparse
import json
from pathlib import Path
from cheroot.wsgi import Server
from wsgidav.wsgidav_app import WsgiDAVApp

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--port", type=int, default=0)
args = parser.parse_args()
args.root.mkdir(parents=True, exist_ok=True)
app = WsgiDAVApp({
    "provider_mapping": {"/": str(args.root.resolve())},
    "simple_dc": {"user_mapping": {"*": {"sync-test": {"password": "fixture-login", "roles": []}}}},
    "http_authenticator": {"accept_basic": True, "accept_digest": False, "default_to_digest": False},
    "verbose": 1,
    "logging": {"enable": False},
})
server = Server(("127.0.0.1", args.port), app)
server.prepare()
print(json.dumps({"endpoint": f"http://127.0.0.1:{server.socket.getsockname()[1]}/"}), flush=True)
try:
    server.serve()
finally:
    server.stop()
