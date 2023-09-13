import 'package:flutter/material.dart';

class MiddleEllipsisedText extends StatelessWidget {
  final String value;
  final TextStyle? style;

  const MiddleEllipsisedText(this.value, {this.style, super.key});

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
        Text(value.substring(0, 3), style: style),
        Expanded(
            child: Text(
          value.substring(3, value.length - 3),
          softWrap: false,
          overflow: TextOverflow.fade,
          style: style,
        )),
        Text('...', overflow: TextOverflow.fade, softWrap: false, style: style),
        Text(value.substring(value.length - 3, value.length), style: style),
      ]);
    } else {
      return Text(value, style: style);
    }
  }
}
