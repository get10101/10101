import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';

class TenTenOneSwitch extends StatefulWidget {
  final bool value;
  final ValueChanged<bool> onChanged;
  final bool showDisabled;
  final bool isDisabled;

  const TenTenOneSwitch(
      {Key? key,
      required this.value,
      required this.onChanged,
      this.showDisabled = true,
      this.isDisabled = false})
      : super(key: key);

  @override
  State<TenTenOneSwitch> createState() => _TenTenOneSwitchState();
}

class _TenTenOneSwitchState extends State<TenTenOneSwitch> with SingleTickerProviderStateMixin {
  Animation? _circleAnimation;
  AnimationController? _animationController;

  @override
  void initState() {
    super.initState();
    _animationController =
        AnimationController(vsync: this, duration: const Duration(milliseconds: 60));
    _circleAnimation = AlignmentTween(
            begin: widget.value ? Alignment.centerLeft : Alignment.centerRight,
            end: widget.value ? Alignment.centerRight : Alignment.centerLeft)
        .animate(CurvedAnimation(parent: _animationController!, curve: Curves.linear));
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _animationController!,
      builder: (context, child) {
        return GestureDetector(
          onTap: widget.isDisabled
              ? () => {}
              : () {
                  if (_animationController!.isCompleted) {
                    _animationController!.reverse();
                  } else {
                    _animationController!.forward();
                  }
                  widget.value == false ? widget.onChanged(true) : widget.onChanged(false);
                },
          child: Container(
            width: 50.0,
            height: 30.0,
            decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(24.0),
                color: _circleAnimation!.value == Alignment.centerLeft
                    ? tenTenOnePurple.shade300
                    : widget.showDisabled
                        ? tenTenOnePurple.shade100
                        : tenTenOnePurple.shade300),
            child: Padding(
              padding: const EdgeInsets.only(top: 6.0, bottom: 6.0, left: 5.0, right: 5.0),
              child: Container(
                alignment: widget.value
                    ? ((Directionality.of(context) == TextDirection.rtl)
                        ? Alignment.centerLeft
                        : Alignment.centerRight)
                    : ((Directionality.of(context) == TextDirection.rtl)
                        ? Alignment.centerRight
                        : Alignment.centerLeft),
                child: Container(
                  width: 20.0,
                  height: 20.0,
                  decoration: const BoxDecoration(shape: BoxShape.circle, color: Colors.white),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
