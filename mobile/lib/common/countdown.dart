import 'dart:async';

import 'package:flutter/material.dart';

class Countdown extends StatefulWidget {
  final int start;

  const Countdown({super.key, required this.start});

  @override
  CountdownState createState() => CountdownState();
}

class CountdownState extends State<Countdown> {
  Timer? _timer;
  int _start = 0;

  @override
  void initState() {
    super.initState();
    _start = widget.start;
    _timer = Timer.periodic(const Duration(seconds: 1), (timer) {
      setState(() {
        if (_start > 0) {
          _start--;
        } else {
          _timer?.cancel();
        }
      });
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  String get _formattedTime {
    final minutes = _start ~/ 60;
    final seconds = _start % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }

  @override
  Widget build(BuildContext context) {
    return Text(
      _formattedTime,
      style: const TextStyle(fontSize: 14),
    );
  }
}
