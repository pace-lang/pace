import 'dart:io';

void main() async {
  var server = await HttpServer.bind('127.0.0.1', 3002);
  print('Dart HTTP server listening on http://127.0.0.1:3002');
  
  await for (HttpRequest request in server) {
    if (request.uri.path == '/json') {
      request.response
        ..statusCode = HttpStatus.ok
        ..headers.contentType = ContentType.json
        ..write('{"status": "ok", "language": "dart"}')
        ..close();
    } else {
      request.response
        ..statusCode = HttpStatus.ok
        ..headers.contentType = ContentType.text
        ..write('Hello from Dart Server!')
        ..close();
    }
  }
}
