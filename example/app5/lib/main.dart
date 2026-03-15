// ignore_for_file: avoid_print

/// Flutter Demon Example — Pre-App Backend Demo
///
/// Simple app that fetches todos from a local Python backend server.
/// The backend is configured as a start_before_app custom source in
/// .fdemon/config.toml, so fdemon waits for it to be healthy before
/// launching this Flutter app.

import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

const backendUrl = 'http://127.0.0.1:8085';

void main() {
  print('Flutter app starting — backend should already be ready');
  runApp(const TodoApp());
}

class TodoApp extends StatelessWidget {
  const TodoApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Pre-App Backend Demo',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.teal),
        useMaterial3: true,
      ),
      home: const TodoPage(),
    );
  }
}

class TodoPage extends StatefulWidget {
  const TodoPage({super.key});

  @override
  State<TodoPage> createState() => _TodoPageState();
}

class _TodoPageState extends State<TodoPage> {
  List<Map<String, dynamic>> _todos = [];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _fetchTodos();
  }

  Future<void> _fetchTodos() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final response = await http.get(Uri.parse('$backendUrl/api/todos'));
      print('GET /api/todos -> ${response.statusCode}');

      if (response.statusCode == 200) {
        final List<dynamic> data = jsonDecode(response.body);
        setState(() {
          _todos = data.cast<Map<String, dynamic>>();
          _loading = false;
        });
      } else {
        setState(() {
          _error = 'Server returned ${response.statusCode}';
          _loading = false;
        });
      }
    } catch (e) {
      print('Error fetching todos: $e');
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  Future<void> _addTodo() async {
    final controller = TextEditingController();
    final title = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('New Todo'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(hintText: 'What needs doing?'),
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(controller.text),
            child: const Text('Add'),
          ),
        ],
      ),
    );

    if (title == null || title.trim().isEmpty) return;

    try {
      final response = await http.post(
        Uri.parse('$backendUrl/api/todos'),
        headers: {'Content-Type': 'application/json'},
        body: jsonEncode({'title': title.trim()}),
      );
      print('POST /api/todos -> ${response.statusCode}');
      _fetchTodos();
    } catch (e) {
      print('Error adding todo: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Backend Demo'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _fetchTodos,
            tooltip: 'Refresh',
          ),
        ],
      ),
      body: _buildBody(),
      floatingActionButton: FloatingActionButton(
        onPressed: _addTodo,
        child: const Icon(Icons.add),
      ),
    );
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.error_outline, size: 48, color: Colors.red.shade300),
            const SizedBox(height: 16),
            Text('Failed to reach backend', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            Text(_error!, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 16),
            ElevatedButton(onPressed: _fetchTodos, child: const Text('Retry')),
          ],
        ),
      );
    }

    if (_todos.isEmpty) {
      return const Center(child: Text('No todos yet. Tap + to add one.'));
    }

    return ListView.builder(
      itemCount: _todos.length,
      itemBuilder: (context, index) {
        final todo = _todos[index];
        return ListTile(
          leading: Icon(
            todo['done'] == true ? Icons.check_circle : Icons.circle_outlined,
            color: todo['done'] == true ? Colors.green : null,
          ),
          title: Text(
            todo['title'] ?? '',
            style: TextStyle(
              decoration: todo['done'] == true ? TextDecoration.lineThrough : null,
            ),
          ),
        );
      },
    );
  }
}
