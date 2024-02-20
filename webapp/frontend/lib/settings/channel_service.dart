import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/settings/dlc_channel.dart';

class ChannelService {
  const ChannelService();

  Future<List<DlcChannel>> getChannelDetails() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/channels'));

    if (response.statusCode == 200) {
      List<dynamic> data = jsonDecode(response.body);
      return data.map((item) => DlcChannel.fromJson(item)).toList();
    } else {
      throw FlutterError("Failed to fetch seed phrase");
    }
  }

  Future<void> closeChannel(bool force) async {
    final queryParams = {'force': '$force'};
    final response = await HttpClientManager.instance
        .delete(Uri(path: '/api/channels', queryParameters: queryParams));

    logger.i("${response.body} ${response.statusCode}");
    if (response.statusCode == 200) {
      logger.i("Successfully closed channel");
    } else {
      throw FlutterError(
          "Failed to close channel. HTTP${response.statusCode}: ${response.reasonPhrase}. ${response.body}");
    }
  }
}
