import 'package:flutter/material.dart';

import 'amount_text.dart';
import 'fiat_text.dart';

enum ValueType { amount, fiat, percentage, contracts }

class ValueDataRow extends StatelessWidget {
  final ValueType type;
  final String label;
  final dynamic value;
  final TextStyle valueTextStyle;
  final TextStyle labelTextStyle;

  const ValueDataRow(
      {super.key,
      required this.type,
      required this.value,
      required this.label,
      this.valueTextStyle = const TextStyle(),
      this.labelTextStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
    Widget widget;

    switch (type) {
      case ValueType.amount:
        widget = AmountText(
          amount: value,
          textStyle: valueTextStyle,
        );
        break;
      case ValueType.fiat:
        widget = FiatText(amount: value, textStyle: valueTextStyle);
        break;
      case ValueType.percentage:
        widget = Text("$value %", style: valueTextStyle);
        break;
      case ValueType.contracts:
        widget = Text("$value contracts", style: valueTextStyle);
        break;
    }

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(
          label,
          style: labelTextStyle,
        ),
        widget
      ],
    );
  }
}
