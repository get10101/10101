import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:get_10101/common/domain/dlc_channel.dart';
import 'package:get_10101/logger/logger.dart';

enum ChannelStatus {
  unknown,
  notOpen,
  closing,
  withPosition,
  open,
  renewing,
  settling,
}

class DlcChannelChangeNotifier extends ChangeNotifier implements Subscriber {
  final DlcChannelService dlcChannelService;

  Map<String, DlcChannel> channels = {};

  DlcChannelChangeNotifier(this.dlcChannelService);

  Future<void> initialize() async {
    List<DlcChannel> channels = await dlcChannelService.getDlcChannels();
    for (DlcChannel channel in channels) {
      this.channels[channel.id] = channel;
    }

    notifyListeners();
  }

  Future<void> deleteDlcChannel(String dlcChannelId) async {
    await dlcChannelService.deleteDlcChannel(dlcChannelId);
  }

  List<SignedDlcChannel> getAllSignedDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.signed)
        .map((channel) => channel as SignedDlcChannel)
        .toList();
  }

  List<OfferedDlcChannel> getAllOfferedDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.offered)
        .map((channel) => channel as OfferedDlcChannel)
        .toList();
  }

  List<AcceptedDlcChannel> getAllAcceptedDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.accepted)
        .map((channel) => channel as AcceptedDlcChannel)
        .toList();
  }

  List<CancelledDlcChannel> getAllCancelledDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.cancelled)
        .map((channel) => channel as CancelledDlcChannel)
        .toList();
  }

  List<ClosingDlcChannel> getAllClosingDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.closing)
        .map((channel) => channel as ClosingDlcChannel)
        .toList();
  }

  List<SettledClosingDlcChannel> getAllSettledClosingDlcChannels() {
    return channels.values
        .where((channel) => channel.state == ChannelState.settledClosing)
        .map((channel) => channel as SettledClosingDlcChannel)
        .toList();
  }

  List<ClosedDlcChannel> getAllClosedDlcChannels() {
    return channels.values
        .where((channel) => [
              ChannelState.closed,
              ChannelState.counterClosed,
              ChannelState.collaborativelyClosed,
            ].contains(channel.state))
        .map((channel) => channel as ClosedDlcChannel)
        .toList();
  }

  List<DlcChannel> getAllOtherDlcChannels() {
    return channels.values
        .where((channel) => [
              ChannelState.closedPunished,
              ChannelState.failedAccept,
              ChannelState.failedSign,
            ].contains(channel.state))
        .toList();
  }

  ChannelStatus getChannelStatus() {
    if (!channels.values.any((c) => c.state == ChannelState.signed)) {
      return ChannelStatus.notOpen;
    }

    final channel = channels.values.firstWhere((channel) => channel.state == ChannelState.signed)
        as SignedDlcChannel;
    switch (channel.signedState) {
      case SignedChannelState.established:
        return ChannelStatus.withPosition;
      case SignedChannelState.settledOffered:
        return ChannelStatus.settling;
      case SignedChannelState.settledReceived:
        return ChannelStatus.settling;
      case SignedChannelState.settledAccepted:
        return ChannelStatus.settling;
      case SignedChannelState.settledConfirmed:
        return ChannelStatus.settling;
      case SignedChannelState.settled:
        return ChannelStatus.open;
      case SignedChannelState.renewOffered:
        return ChannelStatus.renewing;
      case SignedChannelState.renewAccepted:
        return ChannelStatus.renewing;
      case SignedChannelState.renewConfirmed:
        return ChannelStatus.renewing;
      case SignedChannelState.renewFinalized:
        return ChannelStatus.renewing;
      case SignedChannelState.closing:
        return ChannelStatus.notOpen;
      case SignedChannelState.settledClosing:
        return ChannelStatus.notOpen;
      case SignedChannelState.collaborativeCloseOffered:
        return ChannelStatus.unknown;
    }
  }

  bool hasDlcChannel() {
    return channels.values.any((channel) => channel.state == ChannelState.signed);
  }

  bool hasDlcChannelWithoutPosition() {
    return getAllSignedDlcChannels().any((channel) {
      return [
        SignedChannelState.settled,
        SignedChannelState.settledAccepted,
        SignedChannelState.settledConfirmed
      ].contains(channel.signedState);
    });
  }

  bool canForceClose() {
    return hasDlcChannel();
  }

  /// Whether the current DLC channel is closing or not.
  bool isClosing() {
    return channels.values.any((channel) =>
        channel.state == ChannelState.closing ||
        (channel is SignedDlcChannel && channel.signedState == SignedChannelState.closing));
  }

  bool hasOpenPosition() {
    return getAllSignedDlcChannels().any((channel) {
      return [
        SignedChannelState.established,
        SignedChannelState.renewAccepted,
        SignedChannelState.renewConfirmed,
        SignedChannelState.renewFinalized,
        SignedChannelState.renewOffered,
        SignedChannelState.settledOffered
      ].contains(channel.signedState);
    });
  }

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_DlcChannelEvent) {
      DlcChannel channel = DlcChannel.fromApi(event.field0);

      if (channels.containsKey(event.field0.referenceId)) {
        // if we can find the channel on the reference id we can remove it now as we have the correct
        // channel id
        channels.remove(event.field0.referenceId);
      } else if (event.field0.channelState is bridge.ChannelState_Offered) {
        // if the channel state is in offered we save the channel on the reference id as the channel
        // id will change.
        channels[event.field0.referenceId] = channel;
      } else {
        channels[channel.id] = channel;
      }

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}
