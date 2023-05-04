import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';

class ModalBottomSheetInfo extends StatelessWidget {
  final String infoText;
  final String buttonText;
  final EdgeInsets padding;

  const ModalBottomSheetInfo(
      {super.key,
      required this.infoText,
      required this.buttonText,
      this.padding = const EdgeInsets.all(8.0)});

  @override
  Widget build(BuildContext context) {
    // TODO: implement build
    return IconButton(
        onPressed: () {
          showModalBottomSheet<void>(
            shape: const RoundedRectangleBorder(
              borderRadius: BorderRadius.vertical(
                top: Radius.circular(20),
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
                    // TODO: Add link to FAQ
                    Text(infoText),
                    ElevatedButton(onPressed: () => Navigator.pop(context), child: Text(buttonText))
                  ],
                ),
              );
            },
          );
        },
        padding: padding,
        constraints: const BoxConstraints(),
        icon: Icon(
          Icons.info,
          color: tenTenOnePurple.shade200,
        ));
  }
}
