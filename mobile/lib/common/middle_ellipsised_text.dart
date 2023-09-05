import 'package:flutter/material.dart';

class MiddleEllipsisedText extends StatelessWidget {
  final String value;

  const MiddleEllipsisedText(this.value, {super.key});

  @override
  Widget build(BuildContext context) {
    if (value.length > 6) {
      return Row(children: [
        Text(value.substring(0, 3)),
        Flexible(
            child: Text(
          value.substring(3, value.length - 3),
          overflow: TextOverflow.ellipsis,
        )),
        Text(value.substring(value.length - 3, value.length)),
      ]);
    } else {
      return Text(value);
    }
  }
}
