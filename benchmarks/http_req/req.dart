import 'dart:io';

void main() async {
  var url = Uri.parse('https://jsonplaceholder.typicode.com/todos/1');
  var iterations = 10;
  var successCount = 0;
  
  var client = HttpClient();
  
  var stopwatch = Stopwatch()..start();
  
  for (var i = 0; i < iterations; i++) {
    var request = await client.getUrl(url);
    var response = await request.close();
    
    if (response.statusCode == 200) {
      successCount++;
      // Consume the body
      await response.join();
    }
  }
  
  stopwatch.stop();
  client.close();
  
  print('Dart: $successCount/$iterations successful requests in ${stopwatch.elapsedMilliseconds / 1000}s');
}
