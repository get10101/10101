import 'package:flutter/cupertino.dart';

class ScrollableSafeArea extends StatelessWidget {
  const ScrollableSafeArea({
    required this.child,
    Key? key,
  }) : super(key: key);

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
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
