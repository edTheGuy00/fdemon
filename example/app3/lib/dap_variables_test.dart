// ignore_for_file: avoid_print, unused_field, unused_element, equal_elements_in_set

/// DAP Variables Panel Test Page
///
/// This file creates a rich variety of Dart variable types for manually
/// testing the DAP debugger's variables panel (Phase 6 features).
///
/// HOW TO USE:
///   1. Launch app3 via fdemon DAP
///   2. Set a breakpoint on any line marked "// <-- BREAKPOINT"
///   3. Tap the "Populate Variables" button
///   4. Inspect the variables panel in your IDE
///
/// All test variables are stored as instance fields on the State object,
/// so they remain visible in the debugger (locals would be optimized away).
///
/// What to verify:
///   - Locals scope shows all local variables with correct types
///   - Globals scope shows library-level statics
///   - Custom classes show correct class name (not "PlainInstance instance")
///   - toString() values display inline for PlainInstance types
///   - Getters appear and can be evaluated (with timeout)
///   - Records display as (field1, field2, named: value)
///   - Maps with complex keys show toString() for keys
///   - Long strings are truncated with "…"
///   - Nested objects can be drilled into (evaluateName works)
///   - Enums display correctly
///   - Nullable types show null vs value
///   - WeakReference displays target or null
///   - Exception scope appears when paused at an exception

import 'dart:async';
import 'dart:math';
import 'package:flutter/material.dart';

// ============================================================================
// GLOBALS — should appear in "Globals" scope
// ============================================================================

/// Static counter for globals scope testing.
int globalCounter = 0;

/// Static string for globals scope testing.
const String globalAppName = 'DAP Variables Test';

/// Mutable global list.
final List<String> globalTags = ['flutter', 'dart', 'dap', 'debug'];

/// Global map.
final Map<String, int> globalScores = {
  'alice': 100,
  'bob': 85,
  'charlie': 92,
};

/// Global enum value.
Priority globalPriority = Priority.high;

// ============================================================================
// ENUMS
// ============================================================================

enum Priority { low, medium, high, critical }

enum ConnectionState { disconnected, connecting, connected, error }

enum HttpMethod { get, post, put, patch, delete }

// ============================================================================
// CLASSES — test class names, toString(), getters, inheritance
// ============================================================================

/// Simple class with toString override — tests toString display.
class User {
  final String name;
  final int age;
  final String email;
  final List<String> roles;
  final Address? address;

  const User({
    required this.name,
    required this.age,
    required this.email,
    this.roles = const [],
    this.address,
  });

  /// Getter — should be evaluatable in variables panel.
  String get displayName => '$name ($age)';

  /// Getter that does some computation.
  bool get isAdmin => roles.contains('admin');

  /// Getter returning nullable.
  String? get primaryRole => roles.isEmpty ? null : roles.first;

  @override
  String toString() => 'User($name, age=$age)';
}

/// Nested class for drill-down testing.
class Address {
  final String street;
  final String city;
  final String country;
  final GeoPoint? coordinates;

  const Address({
    required this.street,
    required this.city,
    required this.country,
    this.coordinates,
  });

  /// Getter on nested class.
  String get fullAddress => '$street, $city, $country';

  @override
  String toString() => '$city, $country';
}

/// Deeply nested class.
class GeoPoint {
  final double latitude;
  final double longitude;

  const GeoPoint(this.latitude, this.longitude);

  /// Getter with computation.
  double get distanceFromOrigin =>
      sqrt(latitude * latitude + longitude * longitude);

  @override
  String toString() => '($latitude, $longitude)';
}

/// Class with private fields and multiple getters — tests getter evaluation
/// through class hierarchy.
class ApiResponse {
  final int statusCode;
  final Map<String, String> headers;
  final String? body;
  final Duration elapsed;
  final DateTime timestamp;

  ApiResponse({
    required this.statusCode,
    required this.headers,
    this.body,
    required this.elapsed,
    DateTime? timestamp,
  }) : timestamp = timestamp ?? DateTime.now();

  bool get isSuccess => statusCode >= 200 && statusCode < 300;
  bool get isError => statusCode >= 400;
  bool get hasBody => body != null && body!.isNotEmpty;
  int get bodyLength => body?.length ?? 0;
  String get statusCategory => switch (statusCode) {
        >= 200 && < 300 => 'Success',
        >= 300 && < 400 => 'Redirect',
        >= 400 && < 500 => 'Client Error',
        >= 500 => 'Server Error',
        _ => 'Unknown',
      };

  @override
  String toString() => 'ApiResponse($statusCode, ${elapsed.inMilliseconds}ms)';
}

/// Abstract base class for inheritance testing.
abstract class Shape {
  String get name;
  double get area;
  double get perimeter;

  @override
  String toString() => '$name(area=${area.toStringAsFixed(2)})';
}

class Circle extends Shape {
  final double radius;
  Circle(this.radius);

  @override
  String get name => 'Circle';
  @override
  double get area => pi * radius * radius;
  @override
  double get perimeter => 2 * pi * radius;

  /// Circle-specific getter.
  double get diameter => radius * 2;
}

class Rectangle extends Shape {
  final double width;
  final double height;
  Rectangle(this.width, this.height);

  @override
  String get name => 'Rectangle';
  @override
  double get area => width * height;
  @override
  double get perimeter => 2 * (width + height);

  /// Rectangle-specific getter.
  bool get isSquare => width == height;
  double get diagonal => sqrt(width * width + height * height);
}

/// Class used as a complex map key — tests map key toString().
class MapKeyClass {
  final String id;
  final int version;

  const MapKeyClass(this.id, this.version);

  @override
  String toString() => 'Key($id:v$version)';

  @override
  bool operator ==(Object other) =>
      other is MapKeyClass && other.id == id && other.version == version;

  @override
  int get hashCode => Object.hash(id, version);
}

/// Generic class — tests type parameter display.
class Pair<A, B> {
  final A first;
  final B second;
  const Pair(this.first, this.second);

  @override
  String toString() => 'Pair($first, $second)';
}

/// Class with a slow getter — tests getter evaluation timeout.
class SlowComputation {
  final int value;
  SlowComputation(this.value);

  /// This getter is intentionally slow to test timeout handling.
  /// The DAP server should impose a time budget.
  int get expensiveResult {
    var result = 0;
    for (var i = 0; i < value * 1000; i++) {
      result += i % 7;
    }
    return result;
  }

  /// Normal-speed getter for comparison.
  int get doubleValue => value * 2;

  @override
  String toString() => 'SlowComputation($value)';
}

/// Class with many fields — tests scrolling / large variable counts.
class Config {
  final String appName;
  final String version;
  final int maxRetries;
  final Duration timeout;
  final bool debugMode;
  final bool verboseLogging;
  final String? apiKey;
  final String baseUrl;
  final int port;
  final List<String> allowedOrigins;
  final Map<String, dynamic> features;
  final double rateLimit;
  final Priority logLevel;

  const Config({
    required this.appName,
    required this.version,
    this.maxRetries = 3,
    this.timeout = const Duration(seconds: 30),
    this.debugMode = false,
    this.verboseLogging = false,
    this.apiKey,
    this.baseUrl = 'https://api.example.com',
    this.port = 8080,
    this.allowedOrigins = const ['localhost'],
    this.features = const {},
    this.rateLimit = 100.0,
    this.logLevel = Priority.medium,
  });

  int get fieldCount => 13;
  bool get hasApiKey => apiKey != null;
  String get fullUrl => '$baseUrl:$port';

  @override
  String toString() => 'Config($appName v$version)';
}

// ============================================================================
// CUSTOM EXCEPTION — tests exception scope
// ============================================================================

class AppException implements Exception {
  final String message;
  final String code;
  final Map<String, dynamic>? context;

  AppException(this.message, {this.code = 'UNKNOWN', this.context});

  @override
  String toString() => 'AppException($code): $message';
}

// ============================================================================
// WIDGET
// ============================================================================

class DapVariablesTestPage extends StatefulWidget {
  const DapVariablesTestPage({super.key});

  @override
  State<DapVariablesTestPage> createState() => _DapVariablesTestPageState();
}

class _DapVariablesTestPageState extends State<DapVariablesTestPage> {
  String _status = 'Tap a button to populate variables';

  // ==========================================================================
  // All test variables are instance fields so the Dart VM cannot optimize
  // them away. They are always visible when the debugger pauses on `this`.
  // ==========================================================================

  // --- Primitives ---
  int intVar = 0;
  double doubleVar = 0.0;
  double negativeDouble = 0.0;
  double infinityVar = 0.0;
  double nanVar = 0.0;
  bool boolTrue = false;
  bool boolFalse = false;
  String shortString = '';
  String emptyString = '';
  String unicodeString = '';
  int bigInt = 0;
  String longString = '';
  String multilineString = '';

  // --- Enums ---
  Priority priorityVar = Priority.low;
  ConnectionState connStateVar = ConnectionState.disconnected;
  HttpMethod methodVar = HttpMethod.get;

  // --- Collections ---
  List<int> emptyIntList = [];
  List<int> intList = [];
  List<String> stringList = [];
  List<Object?> mixedList = [];
  List<List<int>> nestedList = [];
  List<String> largeList = [];
  Set<String> emptyStringSet = {};
  Set<int> numberSet = {};
  Set<String> stringSet = {};
  Map<String, int> emptyStringIntMap = {};
  Map<String, int> stringIntMap = {};
  Map<String, dynamic> nestedMap = {};
  Map<MapKeyClass, String> complexKeyMap = {};
  Map<int, String> intKeyMap = {};
  Map<Priority, String> enumKeyMap = {};

  // --- Records ---
  (int, String)? simpleRecord;
  ({int x, int y, String label})? namedRecord;
  (int, {String name, bool active})? mixedRecord;
  ({(String, int) user, List<int> scores})? nestedRecord;
  List<(String, int, bool)> typeAnnotatedRecords = [];

  // --- Custom objects ---
  User? user1;
  User? user2;
  User? userNoAddress;
  ApiResponse? response200;
  ApiResponse? response404;
  ApiResponse? response500;

  // --- Inheritance ---
  Shape? circleShape;
  Shape? rectangleShape;
  List<Shape> shapes = [];

  // --- Generics ---
  Pair<String, int>? pair1;
  Pair<User, ApiResponse>? pair2;
  Pair<Pair<String, int>, Pair<String, int>>? pairOfPairs;

  // --- Nullable ---
  String? nullableStringNull;
  String? nullableStringValue;
  int? nullableIntNull;
  int? nullableIntValue;
  User? nullableUserNull;
  User? nullableUserValue;
  List<int?>? nullableListOfNullable;

  // --- WeakReference ---
  Object? weakTarget;
  WeakReference<Object>? weakRef;

  // --- Date/Time ---
  DateTime? nowVar;
  DateTime? epochVar;
  Duration durationVar = Duration.zero;

  // --- Regex ---
  RegExp? emailRegex;
  RegExp? phoneRegex;

  // --- Type system ---
  Type intType = int;
  Type stringType = String;

  // --- Futures ---
  Completer<String>? completerVar;
  Future<String>? completedFuture;
  Future<String>? pendingFuture;

  // --- Complex objects ---
  Config? configVar;
  SlowComputation? slowObj;
  List<User> usersList = [];
  List<ApiResponse> responsesList = [];
  Map<String, User> userMap = {};
  Map<String, dynamic> deepNest = {};

  void _populateVariables() {
    // ========================================================================
    // PRIMITIVES
    // ========================================================================
    intVar = 42;
    doubleVar = 3.14159265358979;
    negativeDouble = -273.15;
    infinityVar = double.infinity;
    nanVar = double.nan;
    boolTrue = true;
    boolFalse = false;
    shortString = 'hello';
    emptyString = '';
    unicodeString = 'Hello 🌍 世界 مرحبا';
    bigInt = 9007199254740992; // 2^53
    longString =
        'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '
        'Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. '
        'Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris '
        'nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in '
        'reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla '
        'pariatur. Excepteur sint occaecat cupidatat non proident.';
    multilineString = 'Line 1\nLine 2\nLine 3\n';

    // ========================================================================
    // ENUMS
    // ========================================================================
    priorityVar = Priority.critical;
    connStateVar = ConnectionState.connected;
    methodVar = HttpMethod.post;

    // ========================================================================
    // LISTS
    // ========================================================================
    emptyIntList = <int>[];
    intList = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    stringList = ['alpha', 'beta', 'gamma', 'delta', 'epsilon'];
    mixedList = <Object?>[42, 'hello', true, 3.14, null, Priority.high];
    nestedList = [
      [1, 2, 3],
      [4, 5, 6],
      [7, 8, 9],
    ];
    largeList = List.generate(100, (i) => 'item_$i');

    // ========================================================================
    // SETS
    // ========================================================================
    emptyStringSet = <String>{};
    numberSet = {1, 2, 3, 5, 8, 13, 21, 34};
    stringSet = {'dart', 'flutter', 'rust', 'dap'};

    // ========================================================================
    // MAPS
    // ========================================================================
    emptyStringIntMap = <String, int>{};
    stringIntMap = {'one': 1, 'two': 2, 'three': 3, 'four': 4};
    nestedMap = {
      'users': {
        'alice': {'age': 30, 'role': 'admin'},
        'bob': {'age': 25, 'role': 'user'},
      },
      'settings': {
        'theme': 'dark',
        'language': 'en',
      },
    };
    complexKeyMap = <MapKeyClass, String>{
      const MapKeyClass('auth', 1): 'basic',
      const MapKeyClass('auth', 2): 'oauth2',
      const MapKeyClass('cache', 1): 'redis',
      const MapKeyClass('db', 3): 'postgres',
    };
    intKeyMap = {0: 'zero', 1: 'one', 2: 'two', 100: 'hundred'};
    enumKeyMap = {
      Priority.low: 'background tasks',
      Priority.medium: 'normal operations',
      Priority.high: 'user-facing',
      Priority.critical: 'system alerts',
    };

    // ========================================================================
    // RECORDS (Dart 3)
    // ========================================================================
    simpleRecord = (1, 'hello');
    namedRecord = (x: 10, y: 20, label: 'origin');
    mixedRecord = (42, name: 'alice', active: true);
    nestedRecord = (
      user: ('bob', 30),
      scores: [95, 87, 92],
    );
    typeAnnotatedRecords = [
      ('a', 1, true),
      ('b', 2, false),
      ('c', 3, true),
    ];

    // ========================================================================
    // CUSTOM OBJECTS — test class names, toString, getters
    // ========================================================================
    user1 = User(
      name: 'Alice Johnson',
      age: 32,
      email: 'alice@example.com',
      roles: ['admin', 'developer'],
      address: const Address(
        street: '123 Main St',
        city: 'London',
        country: 'UK',
        coordinates: GeoPoint(51.5074, -0.1278),
      ),
    );
    user2 = User(
      name: 'Bob Smith',
      age: 28,
      email: 'bob@example.com',
      roles: ['viewer'],
    );
    userNoAddress = User(
      name: 'Charlie',
      age: 45,
      email: 'charlie@example.com',
    );
    response200 = ApiResponse(
      statusCode: 200,
      headers: {
        'content-type': 'application/json',
        'x-request-id': 'abc-123',
        'cache-control': 'no-cache',
      },
      body: '{"status": "ok", "data": [1, 2, 3]}',
      elapsed: const Duration(milliseconds: 142),
    );
    response404 = ApiResponse(
      statusCode: 404,
      headers: {'content-type': 'text/plain'},
      body: 'Not Found',
      elapsed: const Duration(milliseconds: 8),
    );
    response500 = ApiResponse(
      statusCode: 500,
      headers: {},
      elapsed: const Duration(seconds: 2, milliseconds: 350),
    );

    // ========================================================================
    // INHERITANCE — tests getter evaluation through hierarchy
    // ========================================================================
    circleShape = Circle(5.0);
    rectangleShape = Rectangle(4.0, 6.0);
    shapes = <Shape>[
      Circle(1.0),
      Rectangle(2.0, 3.0),
      Circle(10.0),
      Rectangle(5.0, 5.0),
    ];

    // ========================================================================
    // GENERICS
    // ========================================================================
    pair1 = Pair<String, int>('age', 42);
    pair2 = Pair<User, ApiResponse>(user1!, response200!);
    pairOfPairs = Pair(Pair('a', 1), Pair('b', 2));

    // ========================================================================
    // NULLABLE TYPES
    // ========================================================================
    nullableStringNull = null;
    nullableStringValue = 'I have a value';
    nullableIntNull = null;
    nullableIntValue = 99;
    nullableUserNull = null;
    nullableUserValue = user1;
    nullableListOfNullable = [1, null, 3, null, 5];

    // ========================================================================
    // WEAK REFERENCE
    // ========================================================================
    weakTarget = Object();
    weakRef = WeakReference(weakTarget!);

    // ========================================================================
    // DATE/TIME
    // ========================================================================
    nowVar = DateTime.now();
    epochVar = DateTime.fromMillisecondsSinceEpoch(0);
    durationVar = const Duration(hours: 2, minutes: 30, seconds: 15);

    // ========================================================================
    // REGEX
    // ========================================================================
    emailRegex = RegExp(r'^[\w.+-]+@[\w-]+\.[\w.]+$');
    phoneRegex = RegExp(r'^\+?[\d\s-]{7,15}$');

    // ========================================================================
    // FUTURES & COMPLETERS — async state
    // ========================================================================
    completerVar = Completer<String>();
    completedFuture = Future.value('done');
    pendingFuture = completerVar!.future;

    // ========================================================================
    // CONFIG — many fields object
    // ========================================================================
    configVar = Config(
      appName: 'Flutter Demon',
      version: '2.1.0',
      maxRetries: 5,
      timeout: const Duration(seconds: 60),
      debugMode: true,
      verboseLogging: true,
      apiKey: 'sk-test-abc123def456',
      baseUrl: 'https://api.flutterdemon.dev',
      port: 9090,
      allowedOrigins: ['localhost', '127.0.0.1', 'flutterdemon.dev'],
      features: {
        'darkMode': true,
        'betaFeatures': false,
        'maxSessions': 9,
        'experimentalDap': true,
      },
      rateLimit: 250.0,
      logLevel: Priority.high,
    );

    // ========================================================================
    // SLOW GETTER — tests timeout
    // ========================================================================
    slowObj = SlowComputation(50);

    // ========================================================================
    // COMPLEX COLLECTIONS — tests variable expansion
    // ========================================================================
    usersList = [user1!, user2!, userNoAddress!];
    responsesList = [response200!, response404!, response500!];
    userMap = {
      for (final u in usersList) u.email: u,
    };

    // ========================================================================
    // DEEPLY NESTED — tests evaluateName construction
    // ========================================================================
    deepNest = {
      'level1': {
        'level2': {
          'level3': {
            'level4': {
              'value': 'deeply nested value',
              'list': [1, 2, 3],
            },
          },
        },
      },
    };

    // Increment global to verify globals scope mutation
    globalCounter++;

    // ====================================================================
    // BREAKPOINT TARGET
    // Set your breakpoint on the next line to inspect all variables above.
    // All variables are instance fields on `this`, so expand `this` in the
    // debugger or look at the Locals scope.
    // ====================================================================
    _onPopulated(); // <-- BREAKPOINT (step into or set BP on next line)
  }

  void _onPopulated() {
    // All variables are now instance fields — inspect `this` in the debugger.
    // You can also see them in the Locals scope as fields of the State object.
    final fieldCount = 65; // approximate number of test fields
    final summary = 'Run #$globalCounter: $fieldCount fields populated';
    print(summary); // <-- BREAKPOINT HERE
    setState(() {
      _status = summary;
    });
  }

  void _triggerException() {
    try {
      throw AppException(
        'Test exception for DAP variables panel',
        code: 'DAP_TEST_001',
        context: {
          'user': 'alice',
          'action': 'test',
          'timestamp': DateTime.now().toIso8601String(),
        },
      );
    } catch (e) {
      print('Caught exception: $e'); // <-- BREAKPOINT HERE (exception scope)
      setState(() {
        _status = 'Exception caught: $e';
      });
    }
  }

  void _triggerUncaughtException() {
    throw AppException(
      'Uncaught exception for DAP testing',
      code: 'DAP_TEST_UNCAUGHT',
      context: {'severity': 'high', 'component': 'dap_variables_test'},
    );
  }

  void _triggerStateError() {
    final emptyList = <int>[];
    try {
      emptyList.first; // Throws StateError
    } catch (e) {
      print('StateError caught: $e'); // <-- BREAKPOINT HERE
      setState(() => _status = 'StateError: $e');
    }
  }

  void _triggerNestedVariables() {
    final address = const Address(
      street: '10 Downing Street',
      city: 'London',
      country: 'United Kingdom',
      coordinates: GeoPoint(51.5034, -0.1276),
    );

    final user = User(
      name: 'Test User',
      age: 40,
      email: 'test@example.com',
      roles: ['admin', 'superuser', 'developer'],
      address: address,
    );

    // Drill-down path: user -> address -> coordinates -> latitude
    // evaluateName should construct: user.address.coordinates.latitude
    final lat = user.address?.coordinates?.latitude;
    final dist = user.address?.coordinates?.distanceFromOrigin;

    print('Nested: lat=$lat, dist=$dist'); // <-- BREAKPOINT HERE
    setState(() {
      _status = 'Nested variables ready. '
          'Drill into user → address → coordinates.';
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('DAP Variables Test'),
        backgroundColor: theme.colorScheme.primaryContainer,
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Card(
              color: theme.colorScheme.surfaceContainerHighest,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Status', style: theme.textTheme.titleSmall),
                    const SizedBox(height: 8),
                    Text(_status, style: theme.textTheme.bodyMedium),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            Text('Variable Population', style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            _ActionButton(
              label: 'Populate All Variables',
              subtitle: 'Sets 65+ instance fields of all types',
              icon: Icons.data_object,
              color: theme.colorScheme.primary,
              onPressed: _populateVariables,
            ),
            const SizedBox(height: 8),
            _ActionButton(
              label: 'Nested Object Drill-Down',
              subtitle: 'User → Address → GeoPoint chain',
              icon: Icons.account_tree,
              color: theme.colorScheme.secondary,
              onPressed: _triggerNestedVariables,
            ),
            const SizedBox(height: 16),
            Text('Exception Scope Testing', style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            _ActionButton(
              label: 'Caught Custom Exception',
              subtitle: 'AppException with code + context map',
              icon: Icons.error_outline,
              color: Colors.orange,
              onPressed: _triggerException,
            ),
            const SizedBox(height: 8),
            _ActionButton(
              label: 'Caught StateError',
              subtitle: 'Standard Dart StateError',
              icon: Icons.warning_amber,
              color: Colors.amber.shade700,
              onPressed: _triggerStateError,
            ),
            const SizedBox(height: 8),
            _ActionButton(
              label: 'Uncaught Exception',
              subtitle: 'Throws without catch — debugger pauses',
              icon: Icons.dangerous,
              color: Colors.red,
              onPressed: _triggerUncaughtException,
            ),
            const Spacer(),
            Text(
              'Global counter: $globalCounter',
              style: theme.textTheme.bodySmall,
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}

class _ActionButton extends StatelessWidget {
  final String label;
  final String subtitle;
  final IconData icon;
  final Color color;
  final VoidCallback onPressed;

  const _ActionButton({
    required this.label,
    required this.subtitle,
    required this.icon,
    required this.color,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return FilledButton.tonal(
      onPressed: onPressed,
      style: FilledButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
      child: Row(
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: TextStyle(fontWeight: FontWeight.w600)),
                Text(
                  subtitle,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
          Icon(Icons.chevron_right, color: color),
        ],
      ),
    );
  }
}
