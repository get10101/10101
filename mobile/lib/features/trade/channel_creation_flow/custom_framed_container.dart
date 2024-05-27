import 'package:flutter/material.dart';

class CustomFramedContainer extends StatelessWidget {
  final String text;
  final Widget child;

  const CustomFramedContainer({super.key, required this.text, required this.child});

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: <Widget>[
        Container(
          width: double.infinity,
          margin: const EdgeInsets.fromLTRB(0, 20, 0, 5),
          padding: const EdgeInsets.only(bottom: 10),
          decoration: BoxDecoration(
            color: Colors.grey.shade100,
            border: Border.all(color: Colors.grey, width: 1),
            borderRadius: BorderRadius.circular(8),
            shape: BoxShape.rectangle,
          ),
          child: child,
        ),
        Positioned(
          left: 5,
          top: 8,
          child: Container(
            padding: const EdgeInsets.only(bottom: 5, left: 5, right: 5),
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: FractionalOffset.topCenter,
                end: FractionalOffset.bottomCenter,
                stops: const [0.28, 0.28],
                colors: [
                  Colors.transparent,
                  Colors.grey.shade100,
                ],
              ),
              shape: BoxShape.rectangle,
            ),
            child: Text(
              text,
              style: const TextStyle(color: Colors.black, fontSize: 16),
            ),
          ),
        ),
      ],
    );
  }
}
