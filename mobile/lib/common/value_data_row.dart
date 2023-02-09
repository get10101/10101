import 'package:flutter/material.dart';

import 'amount_text.dart';
import 'fiat_text.dart';

enum ValueType { amount, fiat, percentage }

class ValueDataRow extends StatelessWidget {
  final ValueType type;
  final String label;
  final dynamic value;

  const ValueDataRow({super.key, required this.type, required this.value, required this.label});

  @override
  Widget build(BuildContext context) {
    Widget widget;

    switch (type) {
      case ValueType.amount:
        widget = AmountText(amount: value);
        break;
      case ValueType.fiat:
        widget = FiatText(amount: value);
        break;
      case ValueType.percentage:
        widget = Text("$value %");
        break;
    }

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [Text(label), widget],
    );
  }
}
