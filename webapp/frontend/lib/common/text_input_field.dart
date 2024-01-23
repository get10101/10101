import 'package:flutter/material.dart';

class TextInputField extends StatelessWidget {
  const TextInputField(
      {super.key,
      this.enabled = true,
      this.label = '',
      this.hint = '',
      this.onChanged,
      this.onSubmitted,
      required this.value,
      this.controller,
      this.validator,
      this.decoration,
      this.style,
      this.obscureText = false,
      this.onTap});

  final TextEditingController? controller;
  final TextStyle? style;
  final String value;
  final bool enabled;
  final String label;
  final String hint;
  final Function(String)? onChanged;
  final Function(String)? onSubmitted;
  final Function()? onTap;
  final InputDecoration? decoration;
  final bool obscureText;

  final String? Function(String?)? validator;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      style: style ?? const TextStyle(color: Colors.black87),
      enabled: enabled,
      controller: controller,
      initialValue: controller != null ? null : value,
      obscureText: obscureText,
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
      onChanged: (value) => {if (onChanged != null) onChanged!(value)},
      onTap: onTap,
      onFieldSubmitted: (value) => {if (onSubmitted != null) onSubmitted!(value)},
      validator: (value) {
        if (validator != null) {
          return validator!(value);
        }

        return null;
      },
    );
  }
}
