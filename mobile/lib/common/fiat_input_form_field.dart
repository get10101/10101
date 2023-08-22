import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'modal_bottom_sheet_info.dart';

class FiatAmountInputField extends StatefulWidget {
  const FiatAmountInputField(
      {super.key,
      required this.controller,
      required this.label,
      required this.hint,
      required this.onChanged,
      required this.value,
      required this.isLoading,
      this.infoText,
      this.validator});

  final double value;
  final TextEditingController controller;
  final String label;
  final String hint;
  final String? infoText;
  final bool isLoading;
  final Function(String) onChanged;

  final String? Function(String?)? validator;

  @override
  State<FiatAmountInputField> createState() => _FiatAmountInputFieldState();
}

class _FiatAmountInputFieldState extends State<FiatAmountInputField> {
  @override
  Widget build(BuildContext context) {
    return TextFormField(
      controller: widget.controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        hintText: widget.hint,
        labelText: widget.label,
        suffixIcon: widget.isLoading
            ? const CircularProgressIndicator()
            : widget.infoText != null
                ? ModalBottomSheetInfo(closeButtonText: "Back...", child: Text(widget.infoText!))
                : null,
      ),
      inputFormatters: <TextInputFormatter>[FilteringTextInputFormatter.digitsOnly],
      onChanged: (value) => widget.onChanged(value),
      validator: (value) {
        if (widget.validator != null) {
          return widget.validator!(value);
        }

        return null;
      },
    );
  }
}
