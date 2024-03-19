import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/numeric_text_formatter.dart';

class AmountInputField extends StatelessWidget {
  /// If `decoration` is passed, then `isLoading`, `hint`, `label`, `infoText`,
  /// and `isLoading` are overriden.
  const AmountInputField({
    super.key,
    this.enabled = true,
    this.label = '',
    this.hint = '',
    this.onChanged,
    this.initialValue,
    this.isLoading = false,
    this.infoText,
    this.controller,
    this.validator,
    this.decoration,
    this.style,
    this.onTap,
    this.textAlign = TextAlign.left,
    this.suffixIcon,
  });

  final TextEditingController? controller;
  final TextStyle? style;
  final Formattable? initialValue;
  final bool enabled;
  final String label;
  final String hint;
  final String? infoText;
  final bool isLoading;
  final Function(String)? onChanged;
  final Function()? onTap;
  final InputDecoration? decoration;
  final TextAlign textAlign;
  final Widget? suffixIcon;

  final String? Function(String?)? validator;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      style: style ?? const TextStyle(color: Colors.black87),
      enabled: enabled,
      controller: controller,
      textAlign: textAlign,
      initialValue: controller != null ? null : initialValue?.formatted(),
      keyboardType: TextInputType.number,
      decoration: decoration ??
          InputDecoration(
            border: const OutlineInputBorder(),
            hintText: hint,
            labelText: label,
            labelStyle: const TextStyle(color: tenTenOnePurple),
            filled: true,
            fillColor: enabled ? Colors.white : Colors.grey[50],
            errorStyle: TextStyle(
              color: Colors.red[900],
            ),
            suffixIcon: isLoading ? const CircularProgressIndicator() : suffixIcon,
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
