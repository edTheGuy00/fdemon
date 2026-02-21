// ignore_for_file: dangling_library_doc_comments

/// Flutter Demon Test App - Sample 2
///
/// This app provides complementary test scenarios for Flutter Demon's
/// log viewing features, focusing on Flutter-specific errors and
/// mixed logger usage.

import 'package:flutter/material.dart';
import 'package:logger/logger.dart';
import 'package:talker_flutter/talker_flutter.dart';

import 'logging/mixed_loggers.dart';
import 'errors/flutter_errors.dart';
import 'networking/http_requests.dart';

final logger = Logger(printer: PrettyPrinter(methodCount: 3));
final talker = TalkerFlutter.init();

void main() {
  runApp(const Sample2App());
}

class Sample2App extends StatelessWidget {
  const Sample2App({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Flutter Demon Test App 2',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.teal),
        useMaterial3: true,
      ),
      home: const Sample2TestPage(),
    );
  }
}

class Sample2TestPage extends StatefulWidget {
  const Sample2TestPage({super.key});

  @override
  State<Sample2TestPage> createState() => _Sample2TestPageState();
}

class _Sample2TestPageState extends State<Sample2TestPage> {
  int _counter = 0;
  bool _showOverflow = false;
  bool _showBuildError = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Sample 2 - Flutter Errors'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        actions: [
          IconButton(
            icon: const Icon(Icons.bug_report),
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => TalkerScreen(talker: talker)),
            ),
            tooltip: 'Open Talker Logs',
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          _buildSection('Counter Demo', [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text('Count: $_counter',
                    style: Theme.of(context).textTheme.headlineMedium),
                const SizedBox(width: 16),
                ElevatedButton(
                  onPressed: () {
                    setState(() => _counter++);
                    logger.i('Counter incremented to $_counter');
                  },
                  child: const Text('Increment'),
                ),
              ],
            ),
          ]),
          _buildSection('Network Requests', [
            _buildButton('GET Post', fetchSinglePost),
            _buildButton('GET List', fetchPostsList),
            _buildButton('POST', createPost),
            _buildButton('PUT', updatePost),
            _buildButton('PATCH', patchPost),
            _buildButton('DELETE', deletePost),
            _buildButton('Comments', fetchComments),
            _buildButton('Dog Image', fetchDogImage),
            _buildButton('Cat Fact', fetchCatFact),
            _buildButton('Headers', fetchWithHeaders),
            _buildButton('Delay 2s', fetchDelayed),
            _buildButton('404', fetch404),
            _buildButton('500', fetch500),
            _buildButton('Burst (6x)', burstRequests),
            _buildButton('Run All', runAllRequests),
          ]),
          _buildSection('Mixed Loggers', [
            _buildButton('Mixed Demo', demonstrateMixedLoggers),
            _buildButton('Request Flow', simulateRequestFlow),
            _buildButton('Verbose (20)', () => verboseLogging(20)),
            _buildButton('Verbose (50)', () => verboseLogging(50)),
          ]),
          _buildSection('Log Levels', [
            _buildButton('All Logger', () {
              logger.t('Trace message');
              logger.d('Debug message');
              logger.i('Info message');
              logger.w('Warning message');
              logger.e('Error message');
              logger.f('Fatal message');
            }),
            _buildButton('All Talker', () {
              talker.verbose('Verbose message');
              talker.debug('Debug message');
              talker.info('Info message');
              talker.warning('Warning message');
              talker.error('Error message');
              talker.critical('Critical message');
              talker.info('Success message');
            }),
          ]),
          _buildSection('Flutter Errors', [
            _buildErrorButton('Flutter Error', triggerFlutterError),
            _buildErrorButton('Platform Exception', triggerPlatformException),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: Colors.purple.shade100,
              ),
              onPressed: () => setState(() => _showOverflow = !_showOverflow),
              child: Text(_showOverflow ? 'Hide Overflow' : 'Show Overflow'),
            ),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: Colors.purple.shade100,
              ),
              onPressed: () => setState(() => _showBuildError = !_showBuildError),
              child: Text(_showBuildError ? 'Hide Build Error' : 'Trigger Build Error'),
            ),
          ]),
          if (_showOverflow) const OverflowWidget(),
          if (_showBuildError)
            Builder(builder: (context) {
              try {
                return const BuildErrorWidget();
              } catch (e, st) {
                logger.e('Build error caught', error: e, stackTrace: st);
                return Container(
                  padding: const EdgeInsets.all(16),
                  color: Colors.red.shade100,
                  child: Text('Build error: $e'),
                );
              }
            }),
          _buildSection('Timer Logs', [
            _buildButton('Log Every 1s (5x)', () {
              for (int i = 0; i < 5; i++) {
                Future.delayed(Duration(seconds: i + 1), () {
                  logger.i('Timer log ${i + 1} of 5');
                });
              }
            }),
            _buildButton('Rapid Logs (100)', () {
              for (int i = 0; i < 100; i++) {
                logger.d('Rapid log #$i');
              }
            }),
          ]),
        ],
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () {
          setState(() => _counter++);
          talker.info('FAB pressed, counter: $_counter');
        },
        child: const Icon(Icons.add),
      ),
    );
  }

  Widget _buildSection(String title, List<Widget> children) {
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            Wrap(spacing: 8, runSpacing: 8, children: children),
          ],
        ),
      ),
    );
  }

  Widget _buildButton(String label, VoidCallback onPressed) {
    return ElevatedButton(onPressed: onPressed, child: Text(label));
  }

  Widget _buildErrorButton(String label, VoidCallback errorFunction) {
    return ElevatedButton(
      style: ElevatedButton.styleFrom(
        backgroundColor: Colors.red.shade100,
        foregroundColor: Colors.red.shade900,
      ),
      onPressed: () {
        try {
          errorFunction();
        } catch (e, st) {
          logger.e('Error: $label', error: e, stackTrace: st);
        }
      },
      child: Text(label),
    );
  }
}
