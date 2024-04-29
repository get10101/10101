import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

class FiatText extends StatelessWidget {
  final double amount;
  final TextStyle textStyle;
  final int? decimalPlaces;

  const FiatText(
      {super.key, required this.amount, this.textStyle = const TextStyle(), this.decimalPlaces});

  @override
  Widget build(BuildContext context) {
    String pattern = "#,###,##0";

    String decimalDigits = '#' * (decimalPlaces ?? 0);

    if (decimalDigits.isNotEmpty) {
      pattern = "$pattern.$decimalDigits";
    }

    final formatter = NumberFormat(pattern, "en");

    return Text("\$${formatter.format(amount)}", style: textStyle);
  }
}
