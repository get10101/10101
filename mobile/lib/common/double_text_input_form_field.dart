import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class DoubleTextInputFormField extends StatelessWidget {
  const DoubleTextInputFormField(
      {super.key,
      required this.controller,
      required this.label,
      this.hint,
      this.onChanged,
      this.value,
      this.enabled = true});

  final double? value;
  final TextEditingController controller;
  final String label;
  final String? hint;
  final Function(String)? onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    // handle value changes
    if (value != null) {
      String value = _trim(this.value.toString());

      int offset = controller.selection.base.offset;
      if (offset > value.length) {
        offset = value.length;
      }

      controller.value = controller.value.copyWith(
        text: value.toString(),
        selection: TextSelection.collapsed(offset: offset),
      );
    }

    // ensure initial value set in controller is trimmed as well
    controller.value = controller.value.copyWith(
      text: _trim(controller.value.text),
    );

    return TextFormField(
      enabled: enabled,
      controller: controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        hintText: hint,
        labelText: label,
      ),
      inputFormatters: [FilteringTextInputFormatter.allow(RegExp(r'(^\d*\.?\d*)'))],
      onChanged: (value) => {if (onChanged != null) onChanged!(value)},
    );
  }

  _trim(String value) {
    if (value.endsWith(".0")) {
      value = value.replaceAll(".0", "");
    }

    return value;
  }
}
