library ui_components;

import 'package:flutter/widgets.dart';
import 'package:core/core.dart';

class AppTitle extends StatelessWidget {
  final String text;

  const AppTitle({super.key, required this.text});

  @override
  Widget build(BuildContext context) {
    CoreLogger.log('Rendering AppTitle: $text');
    return Text(
      text,
      textDirection: TextDirection.ltr,
      style: const TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
    );
  }
}
