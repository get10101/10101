import 'package:flutter/material.dart';

ButtonStyle secondaryActionButtonStyle() {
  ColorScheme greyScheme = ColorScheme.fromSwatch(primarySwatch: Colors.grey);

  return IconButton.styleFrom(
    foregroundColor: Colors.black,
    backgroundColor: Colors.grey.shade200,
    disabledBackgroundColor: greyScheme.onSurface.withOpacity(0.12),
    elevation: 0,
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10.0)),
    hoverColor: greyScheme.onPrimary.withOpacity(0.08),
    focusColor: greyScheme.onPrimary.withOpacity(0.12),
    highlightColor: greyScheme.onPrimary.withOpacity(0.12),
    visualDensity: const VisualDensity(horizontal: 0.0, vertical: 1.0),
  );
}

class SecondaryActionButton extends StatelessWidget {
  final IconData icon;
  final String title;

  SecondaryActionButton({
    super.key,
    required this.onPressed,
    required this.title,
    required this.icon,
  });

  final VoidCallback onPressed;
  final ButtonStyle balanceButtonStyle = secondaryActionButtonStyle();

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      style: balanceButtonStyle,
      // Additionally checking the Lightning balance here, as when
      // hot reloading the app the channel info appears to be
      // unknown.
      onPressed: onPressed,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            icon,
            size: 14,
          ),
          const SizedBox(width: 7, height: 40),
          Text(
            title,
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.normal),
          )
        ],
      ),
    );
  }
}
