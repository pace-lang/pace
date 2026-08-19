from http.server import HTTPServer, BaseHTTPRequestHandler
import json

class SimpleHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        
        if self.path == '/json':
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = {"status": "ok", "language": "python"}
            self.wfile.write(json.dumps(response).encode('utf-8'))
        else:
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            self.wfile.write(b'Hello from Python Server!')
            
    # Disable logging to avoid overhead during benchmark
    def log_message(self, format, *args):
        pass

def run(server_class=HTTPServer, handler_class=SimpleHandler):
    server_address = ('127.0.0.1', 3001)
    httpd = server_class(server_address, handler_class)
    print("Python HTTP server listening on http://127.0.0.1:3001")
    httpd.serve_forever()

if __name__ == '__main__':
    run()
