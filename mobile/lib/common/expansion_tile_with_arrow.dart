import 'package:flutter/material.dart';

class ExpansionTileWithArrow extends StatefulWidget {
  const ExpansionTileWithArrow({
    super.key,
    required this.leading,
    required this.title,
    required this.subtitle,
    required this.trailing,
    required this.children,
    this.expandedCrossAxisAlignment,
    this.expandedAlignment,
  });

  final Widget leading;
  final Widget title;
  final Widget subtitle;
  final Widget trailing;
  final List<Widget> children;
  final CrossAxisAlignment? expandedCrossAxisAlignment;
  final Alignment? expandedAlignment;

  @override
  State<ExpansionTileWithArrow> createState() => _ExpansionTileWithArrowState();
}

class _ExpansionTileWithArrowState extends State<ExpansionTileWithArrow>
    with SingleTickerProviderStateMixin {
  static final Animatable<double> _iconCurve =
      Tween<double>(begin: 0.0, end: 0.5).chain(CurveTween(curve: Curves.easeIn));
  late Animation<double> _iconTurns;
  late AnimationController _animationController;

  @override
  void initState() {
    _animationController =
        AnimationController(duration: const Duration(milliseconds: 200), vsync: this);
    _iconTurns = _animationController.drive(_iconCurve);
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return ExpansionTile(
      leading: widget.leading,
      title: widget.title,
      subtitle: widget.subtitle,
      expandedCrossAxisAlignment: widget.expandedCrossAxisAlignment,
      expandedAlignment: widget.expandedAlignment,
      trailing: FittedBox(
        child: Row(
          children: [
            widget.trailing,
            RotationTransition(
              turns: _iconTurns,
              child: const Icon(Icons.expand_more),
            )
          ],
        ),
      ),
      onExpansionChanged: (bool expanded) {
        setState(() {
          if (expanded) {
            _animationController.forward();
          } else {
            _animationController.reverse();
          }
        });
      },
      children: widget.children,
    );
  }
}
