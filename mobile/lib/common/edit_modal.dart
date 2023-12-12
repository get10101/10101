import 'package:flutter/material.dart';

/// Show a modal designed to allow the user to edit a form field with a keyboard.
/// It is dismissible.
Future<T?> showEditModal<T>(
    {required BuildContext context,
    required double height,
    required Widget Function(BuildContext context, Function(T?) setVal) builder}) {
  T? val;

  return showModalBottomSheet<T>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) {
        return SafeArea(
            child: Padding(
                padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
                // the GestureDetector ensures that we can close the keyboard by tapping into the modal
                child: GestureDetector(
                  onTap: () {
                    FocusScopeNode currentFocus = FocusScope.of(context);

                    if (!currentFocus.hasPrimaryFocus) {
                      currentFocus.unfocus();
                    }
                  },
                  child: SingleChildScrollView(
                    child: SizedBox(
                      // TODO: Find a way to make height dynamic depending on the children size
                      // This is needed because otherwise the keyboard does not push the sheet up correctly
                      height: height,
                      child: builder(context, (newVal) => val = newVal),
                    ),
                  ),
                )));
      }).then((res) => res ?? val);
}
