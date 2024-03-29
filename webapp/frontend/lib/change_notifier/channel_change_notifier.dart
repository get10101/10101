import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/channel_service.dart';
import 'package:get_10101/settings/dlc_channel.dart';

class ChannelChangeNotifier extends ChangeNotifier {
  final ChannelService service;
  late Timer timer;

  List<DlcChannel>? _channels;

  ChannelChangeNotifier(this.service) {
    _refresh();
    Timer.periodic(const Duration(seconds: 2), (timer) async {
      _refresh();
    });
  }

  void _refresh() async {
    try {
      final channels = await service.getChannelDetails();

      if (_channels != channels) {
        _channels = channels;
        // We sort the channel by signed state. A signed channel comes before others. This works because we only have one signed channel currently
        _channels?.sort((a, b) {
          if (a.channelState == ChannelState.signed) {
            return -1;
          } else {
            return 1;
          }
        });
        super.notifyListeners();
      }
    } catch (error) {
      logger.e(error);
    }
  }

  List<DlcChannel>? getChannels() => _channels;

  DlcChannel? getOpenChannel() {
    if (_channels == null) return null;
    try {
      return _channels!.firstWhere(
        (channel) =>
            (channel.signedChannelState == SignedChannelState.established ||
                channel.signedChannelState == SignedChannelState.settled) &&
            channel.channelState == ChannelState.signed,
      );
    } catch (e) {
      // If no element satisfies the condition, we return null
      return null;
    }
  }

  @override
  void dispose() {
    super.dispose();
    timer.cancel();
  }
}
