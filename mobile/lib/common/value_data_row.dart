import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

/// A DateValue displays a `DateTime` with the format `dd.MM.yy-kk:mm`.
class DateValue extends StatelessWidget {
  final DateTime date;
  final TextStyle textStyle;
  const DateValue(this.date, {super.key, this.textStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
    return Text(DateFormat('dd.MM.yy-kk:mm').format(date));
  }
}

/// A LoadingValue displays a widget (given by the `builder` function) when the
/// `value` is not `null`, else displaying a `CircularProgressIndicator`
class LoadingValue<T> extends StatelessWidget {
  final Widget Function(T) builder;
  final T? value;
  const LoadingValue({super.key, required this.value, required this.builder});

  @override
  Widget build(BuildContext context) {
    return this.value != null
        ? builder(this.value as T)
        : const SizedBox(width: 20, height: 20, child: CircularProgressIndicator());
  }
}

class ValueDataRow<T extends Widget> extends StatelessWidget {
  final String label;
  final String sublabel;
  final T value;
  final TextStyle valueTextStyle;
  final TextStyle labelTextStyle;

  const ValueDataRow(
      {super.key,
      required this.value,
      required this.label,
      this.sublabel = "",
      this.valueTextStyle = const TextStyle(),
      this.labelTextStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
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
        value
      ],
    );
  }
}
