import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/http_client.dart';

class SettingsService {
  const SettingsService();

  Future<String> getNodeId() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/node'));

    if (response.statusCode == 200) {
      return response.body;
    } else {
      throw FlutterError("Failed to fetch node id");
    }
  }

  Future<List<String>> getSeedPhrase() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/seed'));

    if (response.statusCode == 200) {
      return jsonDecode(response.body);
    } else {
      throw FlutterError("Failed to fetch seed phrase");
    }
  }
}
