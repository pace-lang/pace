import 'dart:convert';

void main() {
  var source = '{"user":{"id":42,"name":"Aniket","active":true,"balance":1250.75,"email":null,"roles":["developer","maintainer"],"profile":{"age":22,"verified":true,"skills":[{"name":"Rust","level":4},{"name":"Dart","level":5}]}},"projects":[{"name":"Pace","version":0.3,"open_source":true},{"name":"Hadron","version":1.0,"open_source":false}]}';
  
  var stopwatch = Stopwatch()..start();
  for (var i = 0; i < 10000; i++) {
    jsonDecode(source);
  }
  stopwatch.stop();
  
  print('Parsed 10000 times in ${stopwatch.elapsedMilliseconds} ms');
}
