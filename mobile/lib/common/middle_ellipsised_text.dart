import 'package:flutter/material.dart';

class MiddleEllipsisedText extends StatelessWidget {
  final String value;

  const MiddleEllipsisedText(this.value, {super.key});

  @override
  Widget build(BuildContext context) {
    final Size textSize = (TextPainter(
            text: TextSpan(text: value),
            maxLines: 1,
            textScaleFactor: MediaQuery.of(context).textScaleFactor,
            textDirection: TextDirection.ltr)
          ..layout())
        .size;

    if (textSize.width > MediaQuery.sizeOf(context).width - 100) {
      return Row(children: [
        Text(value.substring(0, 3)),
        Expanded(
            child: Text(
          value.substring(3, value.length - 3),
          softWrap: false,
          overflow: TextOverflow.fade,
        )),
        const Text('...', overflow: TextOverflow.fade, softWrap: false),
        Text(value.substring(value.length - 3, value.length)),
      ]);
    } else {
      return Text(value);
    }
  }
}
