import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';

class UsdTextField extends StatefulWidget {
  const UsdTextField(
      {super.key, required this.label, required this.value, this.suffixIcon, this.error});

  final Usd value;
  final String label;
  final Widget? suffixIcon;
  final String? error;

  @override
  State<UsdTextField> createState() => _UsdTextState();
}

class _UsdTextState extends State<UsdTextField> {
  @override
  Widget build(BuildContext context) {
    String value = widget.value.formatted();

    return InputDecorator(
      decoration: InputDecoration(
          contentPadding: const EdgeInsets.fromLTRB(12, 24, 12, 17),
          border: const OutlineInputBorder(),
          labelText: widget.label,
          labelStyle: const TextStyle(color: Colors.black87),
          errorStyle: TextStyle(
            color: Colors.red[900],
          ),
          errorText: widget.error,
          filled: true,
          suffixIcon: widget.suffixIcon,
          fillColor: tenTenOnePurple.shade50.withOpacity(0.3)),
      child: Text(value, style: const TextStyle(fontSize: 16)),
    );
  }
}
