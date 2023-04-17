import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import 'amount_text.dart';
import 'fiat_text.dart';

enum ValueType { date, amount, fiat, percentage, contracts, loading }

class ValueDataRow extends StatelessWidget {
  final ValueType type;
  final String label;
  final String sublabel;
  final dynamic value;
  final TextStyle valueTextStyle;
  final TextStyle labelTextStyle;

  const ValueDataRow(
      {super.key,
      required this.type,
      required this.value,
      required this.label,
      this.sublabel = "",
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
      case ValueType.loading:
        widget = const SizedBox(width: 20, height: 20, child: CircularProgressIndicator());
        break;
      case ValueType.date:
        widget = Text(DateFormat('dd.MM.yy-kk:mm').format(value), style: valueTextStyle);
        break;
    }

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Row(children: [
          Text(
            label,
            style: labelTextStyle,
          ),
          const SizedBox(width: 2),
          Text(sublabel, style: const TextStyle(fontSize: 12, color: Colors.grey)),
        ]),
        widget
      ],
    );
  }
}
