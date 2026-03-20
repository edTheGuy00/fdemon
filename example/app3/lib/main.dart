import 'dart:math';
import 'package:flutter/material.dart';

void main() {
  runApp(const App3());
}

/// Interactive Flutter app for profile mode lag reproduction (Issue #25).
///
/// Features a smooth animation ticker and scrollable list so that periodic
/// freezes caused by aggressive DevTools polling are immediately visible.
/// Run via `cargo run -- example/app3` — the "Profile (Issue #25)" launch
/// config auto-starts in profile mode with max polling pressure.
class App3 extends StatelessWidget {
  const App3({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Flutter Demon App 3',
      theme: ThemeData(
        colorSchemeSeed: Colors.deepPurple,
        useMaterial3: true,
      ),
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> with TickerProviderStateMixin {
  late final AnimationController _spinController;
  late final AnimationController _pulseController;
  final _stopwatch = Stopwatch();
  int _tapCount = 0;
  final _items = List.generate(50, (i) => _RandomItem.generate(i));

  @override
  void initState() {
    super.initState();
    _spinController = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 4),
    )..repeat();
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat(reverse: true);
    _stopwatch.start();
  }

  @override
  void dispose() {
    _spinController.dispose();
    _pulseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('Profile Lag Test'),
        actions: [
          AnimatedBuilder(
            animation: _spinController,
            builder: (_, __) {
              final elapsed = _stopwatch.elapsed;
              final m = elapsed.inMinutes.remainder(60).toString().padLeft(2, '0');
              final s = elapsed.inSeconds.remainder(60).toString().padLeft(2, '0');
              final ms = (elapsed.inMilliseconds.remainder(1000) ~/ 10)
                  .toString()
                  .padLeft(2, '0');
              return Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Center(
                  child: Text(
                    '$m:$s.$ms',
                    style: theme.textTheme.titleMedium?.copyWith(
                      fontFeatures: [const FontFeature.tabularFigures()],
                    ),
                  ),
                ),
              );
            },
          ),
        ],
      ),
      body: Column(
        children: [
          _AnimationBanner(
            spinController: _spinController,
            pulseController: _pulseController,
            tapCount: _tapCount,
          ),
          Expanded(
            child: ListView.builder(
              itemCount: _items.length,
              itemBuilder: (context, index) {
                final item = _items[index];
                return ListTile(
                  leading: CircleAvatar(
                    backgroundColor: item.color,
                    child: Text('${item.id}'),
                  ),
                  title: Text(item.title),
                  subtitle: Text(item.subtitle),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () {
                    setState(() => _tapCount++);
                    ScaffoldMessenger.of(context)
                      ..clearSnackBars()
                      ..showSnackBar(
                        SnackBar(
                          content: Text('Tapped: ${item.title}'),
                          duration: const Duration(seconds: 1),
                        ),
                      );
                  },
                );
              },
            ),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () {
          setState(() {
            _tapCount++;
            _items.insert(0, _RandomItem.generate(_items.length));
          });
        },
        icon: const Icon(Icons.add),
        label: Text('Add item ($_tapCount)'),
      ),
    );
  }
}

class _AnimationBanner extends StatelessWidget {
  const _AnimationBanner({
    required this.spinController,
    required this.pulseController,
    required this.tapCount,
  });

  final AnimationController spinController;
  final AnimationController pulseController;
  final int tapCount;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(24),
      color: theme.colorScheme.primaryContainer,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
        children: [
          // Spinning widget — stutters are obvious here
          AnimatedBuilder(
            animation: spinController,
            builder: (_, child) => Transform.rotate(
              angle: spinController.value * 2 * pi,
              child: child,
            ),
            child: Icon(
              Icons.settings,
              size: 48,
              color: theme.colorScheme.primary,
            ),
          ),
          // Pulsing counter
          AnimatedBuilder(
            animation: pulseController,
            builder: (_, __) {
              final scale = 1.0 + pulseController.value * 0.15;
              return Transform.scale(
                scale: scale,
                child: Column(
                  children: [
                    Text(
                      '$tapCount',
                      style: theme.textTheme.headlineLarge?.copyWith(
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                    Text(
                      'interactions',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
          // Smooth progress ring — frame drops show as jerky motion
          AnimatedBuilder(
            animation: spinController,
            builder: (_, __) => SizedBox(
              width: 48,
              height: 48,
              child: CircularProgressIndicator(
                value: spinController.value,
                strokeWidth: 4,
                color: theme.colorScheme.tertiary,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _RandomItem {
  final int id;
  final String title;
  final String subtitle;
  final Color color;

  const _RandomItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.color,
  });

  static final _rng = Random(42);
  static const _nouns = [
    'Widget', 'Service', 'Controller', 'Provider', 'Repository',
    'Manager', 'Handler', 'Builder', 'Factory', 'Adapter',
  ];
  static const _adjectives = [
    'Async', 'Cached', 'Lazy', 'Reactive', 'Stateful',
    'Immutable', 'Scoped', 'Global', 'Local', 'Shared',
  ];

  static _RandomItem generate(int id) {
    final adj = _adjectives[_rng.nextInt(_adjectives.length)];
    final noun = _nouns[_rng.nextInt(_nouns.length)];
    return _RandomItem(
      id: id,
      title: '$adj$noun',
      subtitle: 'Component #$id — tap to interact',
      color: Colors.primaries[_rng.nextInt(Colors.primaries.length)],
    );
  }
}
