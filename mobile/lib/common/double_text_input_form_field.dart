import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class DoubleTextInputFormField extends StatelessWidget {
  const DoubleTextInputFormField(
      {super.key,
      required this.controller,
      required this.label,
      required this.hint,
      required this.onChanged,
      required this.value});

  final double value;
  final TextEditingController controller;
  final String label;
  final String hint;
  final Function(String) onChanged;

  @override
  Widget build(BuildContext context) {
    String value = this.value.toString();

    if (value.endsWith(".0")) {
      value = value.replaceAll(".0", "");
    }

    int offset = controller.selection.base.offset;
    if (offset > value.length) {
      offset = value.length;
    }

    controller.value = controller.value.copyWith(
      text: value.toString(),
      selection: TextSelection.collapsed(offset: offset),
    );

    return TextFormField(
      controller: controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        hintText: hint,
        labelText: label,
      ),
      inputFormatters: [FilteringTextInputFormatter.allow(RegExp(r'(^\d*\.?\d*)'))],
      onChanged: (value) => onChanged(value),
    );
  }
}
