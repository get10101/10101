import 'package:flutter/material.dart';
import 'package:get_10101/settings/dlc_channel.dart';

class SignedChannelStateLabel extends StatelessWidget {
  final DlcChannel? channel;

  const SignedChannelStateLabel({Key? key, this.channel}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Widget label = _buildLabel("Unknown state", Colors.green.shade300);
    if (channel != null && channel!.signedChannelState != null) {
      label = _buildLabel(channel!.signedChannelState!.nameU, Colors.green.shade300);
    }
    return label;
  }
}

class ChannelStateLabel extends StatelessWidget {
  final DlcChannel? channel;

  const ChannelStateLabel({Key? key, this.channel}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Widget label = _buildLabel("Unknown state", Colors.green.shade300);
    if (channel != null) {
      label = _buildLabel(channel!.channelState.nameU, Colors.green.shade300);
    }
    return label;
  }
}

Widget _buildLabel(String text, Color color) {
  return Container(
    decoration: BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(15),
    ),
    child: Padding(
      padding: const EdgeInsets.all(8.0),
      child: Center(child: Text(text)),
    ),
  );
}
