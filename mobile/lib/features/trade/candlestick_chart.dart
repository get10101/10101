import 'package:flutter/material.dart';

class CandlestickChart extends StatelessWidget {
  const CandlestickChart({super.key});

  @override
  Widget build(BuildContext context) {
    return const SizedBox(
        height: 200,
        child: DecoratedBox(
            decoration: BoxDecoration(color: Colors.grey),
            child: Center(child: Text("Candlestick Chart"))));
  }
}
