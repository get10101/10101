import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/numeric_text_formatter.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';

class SwapAmountInputField extends StatefulWidget {
  const SwapAmountInputField(
      {super.key,
      this.enabled = true,
      this.label,
      this.hint = '',
      this.onChanged,
      this.isLoading = false,
      this.infoText,
      required this.controller,
      this.validator,
      this.border,
      this.style,
      this.denseNoPad = false,
      this.enabledColor,
      this.hoverColor,
      this.autovalidateMode});

  final TextEditingController controller;
  final bool enabled;
  final String? label;
  final TextStyle? style;
  final InputBorder? border;
  final bool denseNoPad;
  final Color? enabledColor;
  final Color? hoverColor;
  final AutovalidateMode? autovalidateMode;
  final String hint;
  final String? infoText;
  final bool isLoading;
  final Function(String)? onChanged;

  final String? Function(String?)? validator;

  @override
  State<SwapAmountInputField> createState() => _SwapAmountInputFieldState();
}

class _SwapAmountInputFieldState extends State<SwapAmountInputField> {
  @override
  Widget build(BuildContext context) {
    return TextFormField(
      style: widget.style ?? const TextStyle(color: Colors.black87),
      autovalidateMode: widget.autovalidateMode,
      enabled: widget.enabled,
      controller: widget.controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        border: widget.border,
        isDense: widget.denseNoPad,
        contentPadding: widget.denseNoPad ? EdgeInsets.zero : null,
        hintText: widget.hint,
        labelText: widget.label,
        labelStyle: const TextStyle(color: Colors.black87),
        filled: true,
        hoverColor: widget.hoverColor,
        fillColor: widget.enabled ? (widget.enabledColor ?? Colors.white) : Colors.grey[50],
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
