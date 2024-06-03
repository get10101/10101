import 'package:flutter/cupertino.dart';

class ScrollableSafeArea extends StatelessWidget {
  const ScrollableSafeArea({
    required this.child,
    this.bottom = true,
    Key? key,
  }) : super(key: key);

  final bool bottom;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
        bottom: bottom,
        child: LayoutBuilder(
            builder: (BuildContext context, BoxConstraints viewportConstraints) =>
                SingleChildScrollView(
                    child: ConstrainedBox(
                  constraints: BoxConstraints(
                    minHeight: viewportConstraints.maxHeight,
                  ),
                  child: IntrinsicHeight(child: child),
                ))));
  }
}
