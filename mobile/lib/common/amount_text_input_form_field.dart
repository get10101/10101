import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/numeric_text_formatter.dart';

import 'domain/model.dart';
import 'modal_bottom_sheet_info.dart';

class AmountInputField extends StatefulWidget {
  const AmountInputField(
      {super.key,
      this.enabled = true,
      required this.label,
      this.hint = '',
      this.onChanged,
      required this.value,
      this.isLoading = false,
      this.infoText,
      this.controller,
      this.validator});

  final TextEditingController? controller;
  final Amount value;
  final bool enabled;
  final String label;
  final String hint;
  final String? infoText;
  final bool isLoading;
  final Function(String)? onChanged;

  final String? Function(String?)? validator;

  @override
  State<AmountInputField> createState() => _AmountInputFieldState();
}

class _AmountInputFieldState extends State<AmountInputField> {
  @override
  Widget build(BuildContext context) {
    return TextFormField(
      style: const TextStyle(color: Colors.black87),
      enabled: widget.enabled,
      controller: widget.controller,
      initialValue: widget.controller != null ? null : widget.value.formatted(),
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        hintText: widget.hint,
        labelText: widget.label,
        labelStyle: const TextStyle(color: Colors.black87),
        filled: true,
        fillColor: widget.enabled ? Colors.white : Colors.grey[50],
        errorStyle: TextStyle(
          color: Colors.red[900],
        ),
        suffixIcon: widget.isLoading
            ? const CircularProgressIndicator()
            : widget.infoText != null
                ? ModalBottomSheetInfo(closeButtonText: "Back", child: Text(widget.infoText!))
                : null,
      ),
      inputFormatters: <TextInputFormatter>[
        FilteringTextInputFormatter.digitsOnly,
        NumericTextFormatter()
      ],
      onChanged: (value) => {if (widget.onChanged != null) widget.onChanged!(value)},
      validator: (value) {
        if (widget.validator != null) {
          return widget.validator!(value);
        }

        return null;
      },
    );
  }
}
