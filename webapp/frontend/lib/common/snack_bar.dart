import 'package:flutter/material.dart';

/// Show a snackbar with the given message
///
/// Extracted as a separate function to ensure consistent style of error messages
void showSnackBar(ScaffoldMessengerState messenger, String message) {
  final snackBar = SnackBar(
    content: Text(message),
    duration: const Duration(milliseconds: 2500),
    behavior: SnackBarBehavior.floating,
    width: 400.0,
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(10.0),
    ),
  );
  messenger.showSnackBar(snackBar);
}
