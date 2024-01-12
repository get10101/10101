import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';

class AmountTextField extends StatefulWidget {
  const AmountTextField({super.key, required this.label, required this.value, this.suffixIcon});

  final Amount value;
  final String label;
  final Widget? suffixIcon;

  @override
  State<AmountTextField> createState() => _AmountTextState();
}

class _AmountTextState extends State<AmountTextField> {
  @override
  Widget build(BuildContext context) {
    String value = widget.value.formatted();

    return InputDecorator(
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        labelText: widget.label,
        labelStyle: const TextStyle(color: Colors.black87),
        filled: true,
        suffixIcon: widget.suffixIcon,
        fillColor: Colors.grey[50],
      ),
      child: Text(value, style: const TextStyle(fontSize: 15)),
    );
  }
}
