import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:json_annotation/json_annotation.dart';

part 'settings_service.g.dart';

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
      var jsonData = jsonDecode(response.body);
      return Seed.fromJson(jsonData).seed;
    } else {
      throw FlutterError("Failed to fetch seed phrase");
    }
  }
}

@JsonSerializable()
class Seed {
  final List<String> seed;

  Seed({required this.seed});

  factory Seed.fromJson(Map<String, dynamic> json) => _$SeedFromJson(json);
}
