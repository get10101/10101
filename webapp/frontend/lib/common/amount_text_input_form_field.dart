import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/amount.dart';
import 'package:get_10101/common/numeric_text_formatter.dart';

class AmountInputField extends StatelessWidget {
  const AmountInputField(
      {super.key,
      this.enabled = true,
      this.label = '',
      this.hint = '',
      this.onChanged,
      required this.value,
      this.controller,
      this.validator,
      this.decoration,
      this.style,
      this.onTap});

  final TextEditingController? controller;
  final TextStyle? style;
  final Amount value;
  final bool enabled;
  final String label;
  final String hint;
  final Function(String)? onChanged;
  final Function()? onTap;
  final InputDecoration? decoration;

  final String? Function(String?)? validator;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      style: style ?? const TextStyle(color: Colors.black87),
      enabled: enabled,
      controller: controller,
      initialValue: controller != null ? null : value.formatted(),
      keyboardType: TextInputType.number,
      decoration: decoration ??
          InputDecoration(
            border: const OutlineInputBorder(),
            hintText: hint,
            labelText: label,
            labelStyle: const TextStyle(color: Colors.black87),
            filled: true,
            fillColor: enabled ? Colors.white : Colors.grey[50],
            errorStyle: TextStyle(
              color: Colors.red[900],
            ),
          ),
      inputFormatters: <TextInputFormatter>[
        FilteringTextInputFormatter.digitsOnly,
        NumericTextFormatter()
      ],
      onChanged: (value) => {if (onChanged != null) onChanged!(value)},
      onTap: onTap,
      validator: (value) {
        if (validator != null) {
          return validator!(value);
        }

        return null;
      },
    );
  }
}
