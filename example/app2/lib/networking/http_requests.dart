import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

import '../main.dart';

// ---------------------------------------------------------------------------
// Public test APIs used:
//   - JSONPlaceholder  (https://jsonplaceholder.typicode.com)
//   - httpbin.org      (https://httpbin.org)
//   - Dog CEO          (https://dog.ceo/api)
//   - Cat Facts        (https://catfact.ninja)
// All are free, open, and require no API keys.
// ---------------------------------------------------------------------------

/// Fire a single GET request and log the result.
Future<void> fetchSinglePost() async {
  logger.i('[Network] GET single post...');
  final response = await http.get(
    Uri.parse('https://jsonplaceholder.typicode.com/posts/1'),
  );
  logger.i('[Network] GET /posts/1 → ${response.statusCode} '
      '(${response.contentLength ?? response.bodyBytes.length} bytes)');
}

/// Fetch a list of posts (larger JSON payload).
Future<void> fetchPostsList() async {
  logger.i('[Network] GET posts list...');
  final response = await http.get(
    Uri.parse('https://jsonplaceholder.typicode.com/posts'),
  );
  final posts = jsonDecode(response.body) as List;
  logger.i('[Network] GET /posts → ${response.statusCode} '
      '— ${posts.length} posts '
      '(${response.bodyBytes.length} bytes)');
}

/// POST a new resource (JSONPlaceholder echoes it back).
Future<void> createPost() async {
  logger.i('[Network] POST new post...');
  final response = await http.post(
    Uri.parse('https://jsonplaceholder.typicode.com/posts'),
    headers: {'Content-Type': 'application/json; charset=UTF-8'},
    body: jsonEncode({
      'title': 'Flutter Demon Test',
      'body': 'Testing network monitoring from fdemon.',
      'userId': 1,
    }),
  );
  logger.i('[Network] POST /posts → ${response.statusCode} '
      '(${response.body.substring(0, 60)}...)');
}

/// PUT (full update) an existing resource.
Future<void> updatePost() async {
  logger.i('[Network] PUT update post...');
  final response = await http.put(
    Uri.parse('https://jsonplaceholder.typicode.com/posts/1'),
    headers: {'Content-Type': 'application/json; charset=UTF-8'},
    body: jsonEncode({
      'id': 1,
      'title': 'Updated by Flutter Demon',
      'body': 'This post was updated via PUT.',
      'userId': 1,
    }),
  );
  logger.i('[Network] PUT /posts/1 → ${response.statusCode}');
}

/// PATCH (partial update) an existing resource.
Future<void> patchPost() async {
  logger.i('[Network] PATCH post...');
  final response = await http.patch(
    Uri.parse('https://jsonplaceholder.typicode.com/posts/1'),
    headers: {'Content-Type': 'application/json; charset=UTF-8'},
    body: jsonEncode({'title': 'Patched by Flutter Demon'}),
  );
  logger.i('[Network] PATCH /posts/1 → ${response.statusCode}');
}

/// DELETE a resource.
Future<void> deletePost() async {
  logger.i('[Network] DELETE post...');
  final response = await http.delete(
    Uri.parse('https://jsonplaceholder.typicode.com/posts/1'),
  );
  logger.i('[Network] DELETE /posts/1 → ${response.statusCode}');
}

/// Fetch comments with query parameters.
Future<void> fetchComments() async {
  logger.i('[Network] GET comments for post 1...');
  final response = await http.get(
    Uri.parse('https://jsonplaceholder.typicode.com/comments?postId=1'),
  );
  final comments = jsonDecode(response.body) as List;
  logger.i('[Network] GET /comments?postId=1 → ${response.statusCode} '
      '— ${comments.length} comments');
}

/// Fetch a random dog image (small JSON, image URL).
Future<void> fetchDogImage() async {
  logger.i('[Network] GET random dog image...');
  final response = await http.get(
    Uri.parse('https://dog.ceo/api/breeds/image/random'),
  );
  final data = jsonDecode(response.body) as Map<String, dynamic>;
  logger.i('[Network] GET dog image → ${response.statusCode} '
      '— ${data['message']}');
}

/// Fetch a random cat fact.
Future<void> fetchCatFact() async {
  logger.i('[Network] GET cat fact...');
  final response = await http.get(
    Uri.parse('https://catfact.ninja/fact'),
  );
  final data = jsonDecode(response.body) as Map<String, dynamic>;
  logger.i('[Network] Cat fact: ${data['fact']}');
}

/// Hit httpbin to echo back custom headers.
Future<void> fetchWithHeaders() async {
  logger.i('[Network] GET with custom headers...');
  final response = await http.get(
    Uri.parse('https://httpbin.org/headers'),
    headers: {
      'X-Flutter-Demon': 'network-test',
      'X-Request-Id': DateTime.now().millisecondsSinceEpoch.toString(),
      'Accept': 'application/json',
    },
  );
  logger.i('[Network] GET /headers → ${response.statusCode} '
      '(${response.bodyBytes.length} bytes)');
}

/// Simulate a delayed response (2 seconds).
Future<void> fetchDelayed() async {
  logger.i('[Network] GET delayed response (2s)...');
  final stopwatch = Stopwatch()..start();
  final response = await http.get(
    Uri.parse('https://httpbin.org/delay/2'),
  );
  stopwatch.stop();
  logger.i('[Network] GET /delay/2 → ${response.statusCode} '
      '— took ${stopwatch.elapsedMilliseconds}ms');
}

/// Request that returns a 404.
Future<void> fetch404() async {
  logger.i('[Network] GET expecting 404...');
  final response = await http.get(
    Uri.parse('https://httpbin.org/status/404'),
  );
  logger.w('[Network] GET /status/404 → ${response.statusCode}');
}

/// Request that returns a 500.
Future<void> fetch500() async {
  logger.i('[Network] GET expecting 500...');
  final response = await http.get(
    Uri.parse('https://httpbin.org/status/500'),
  );
  logger.e('[Network] GET /status/500 → ${response.statusCode}');
}

/// Burst of concurrent requests to multiple endpoints.
Future<void> burstRequests() async {
  logger.i('[Network] Firing burst of 6 concurrent requests...');
  final stopwatch = Stopwatch()..start();
  await Future.wait([
    http.get(Uri.parse('https://jsonplaceholder.typicode.com/posts/1')),
    http.get(Uri.parse('https://jsonplaceholder.typicode.com/posts/2')),
    http.get(Uri.parse('https://jsonplaceholder.typicode.com/users/1')),
    http.get(Uri.parse('https://dog.ceo/api/breeds/image/random')),
    http.get(Uri.parse('https://catfact.ninja/fact')),
    http.get(Uri.parse('https://httpbin.org/get')),
  ]);
  stopwatch.stop();
  logger.i('[Network] Burst complete — 6 requests in '
      '${stopwatch.elapsedMilliseconds}ms');
}

/// Run all request types sequentially for a comprehensive demo.
Future<void> runAllRequests() async {
  logger.i('[Network] === Running all network request demos ===');
  await fetchSinglePost();
  await createPost();
  await updatePost();
  await patchPost();
  await deletePost();
  await fetchComments();
  await fetchDogImage();
  await fetchCatFact();
  await fetchWithHeaders();
  await fetch404();
  await fetch500();
  logger.i('[Network] === All demos complete ===');
}
