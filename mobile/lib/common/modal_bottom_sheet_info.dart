import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';

class ModalBottomSheetInfo extends StatelessWidget {
  final Widget child;
  final String closeButtonText;
  final EdgeInsets infoButtonPadding;

  static const double buttonRadius = 20.0;

  const ModalBottomSheetInfo(
      {super.key,
      required this.child,
      required this.closeButtonText,
      this.infoButtonPadding = const EdgeInsets.all(8.0)});

  @override
  Widget build(BuildContext context) {
    return IconButton(
        onPressed: () {
          showModalBottomSheet<void>(
            shape: const RoundedRectangleBorder(
              borderRadius: BorderRadius.vertical(
                top: Radius.circular(buttonRadius),
              ),
            ),
            clipBehavior: Clip.antiAliasWithSaveLayer,
            useRootNavigator: true,
            context: context,
            builder: (BuildContext context) {
              return Container(
                height: 300,
                padding: const EdgeInsets.all(20.0),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    child,
                    ElevatedButton(
                        onPressed: () => Navigator.pop(context), child: Text(closeButtonText))
                  ],
                ),
              );
            },
          );
        },
        padding: infoButtonPadding,
        constraints: const BoxConstraints(),
        icon: Icon(
          Icons.info,
          color: tenTenOnePurple.shade200,
        ));
  }
}
