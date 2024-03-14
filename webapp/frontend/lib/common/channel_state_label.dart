import 'package:flutter/material.dart';
import 'package:get_10101/settings/dlc_channel.dart';

class SignedChannelStateLabel extends StatelessWidget {
  final DlcChannel? channel;

  const SignedChannelStateLabel({Key? key, this.channel}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Widget label = _buildLabel("Unknown state", Colors.green.shade300);
    if (channel != null && channel!.signedChannelState != null) {
      switch (channel!.signedChannelState) {
        case SignedChannelState.established:
        case SignedChannelState.settled:
          label = _buildLabel("Active", Colors.green.shade300);
          break;
        case SignedChannelState.settledOffered:
        case SignedChannelState.settledReceived:
        case SignedChannelState.settledAccepted:
        case SignedChannelState.settledConfirmed:
        case SignedChannelState.renewOffered:
        case SignedChannelState.renewAccepted:
        case SignedChannelState.renewConfirmed:
        case SignedChannelState.renewFinalized:
          label = _buildLabel("Pending", Colors.green.shade300);
          break;
        case SignedChannelState.closing:
        case SignedChannelState.collaborativeCloseOffered:
          label = _buildLabel("Closing", Colors.orange.shade300);
          break;
        case null:
        // nothing
      }
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
      switch (channel!.channelState) {
        case ChannelState.offered:
          label = _buildLabel("Offered", Colors.grey.shade300);
        case ChannelState.accepted:
          label = _buildLabel("Accepted", Colors.grey.shade300);
        case ChannelState.signed:
          label = _buildLabel("Signed", Colors.grey.shade300);
        case ChannelState.closing:
          label = _buildLabel("Closing", Colors.grey.shade300);
        case ChannelState.closed:
          label = _buildLabel("Closed", Colors.grey.shade300);
        case ChannelState.counterClosed:
          label = _buildLabel("Counter closed", Colors.grey.shade300);
        case ChannelState.closedPunished:
          label = _buildLabel("Closed punished", Colors.grey.shade300);
        case ChannelState.collaborativelyClosed:
          label = _buildLabel("Collaboratively closed", Colors.grey.shade300);
        case ChannelState.failedAccept:
          label = _buildLabel("Failed", Colors.grey.shade300);
        case ChannelState.failedSign:
          label = _buildLabel("Failed", Colors.grey.shade300);
        case ChannelState.cancelled:
          label = _buildLabel("Cancelled", Colors.grey.shade300);
      }
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
