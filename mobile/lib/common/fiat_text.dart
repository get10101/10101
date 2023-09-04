import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

class FiatText extends StatelessWidget {
  final double amount;
  final TextStyle textStyle;

  const FiatText({super.key, required this.amount, this.textStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
    final formatter = NumberFormat("#,###,##0.00", "en");
    return Text("\$${formatter.format(amount)}", style: textStyle);
  }
}
