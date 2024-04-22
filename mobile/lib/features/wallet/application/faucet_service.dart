import 'dart:convert';

import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:http/http.dart' as http;

class FaucetService {
  /// Pay the provided invoice with our faucet
  Future<void> payInvoiceWithFaucet(String bip21Uri, Amount? invoiceAmount) async {
    final split = bip21Uri.split(":");
    final addressAndMaybeAmount = split[1].split("?");
    logger.i("Funding $addressAndMaybeAmount");
    final address = addressAndMaybeAmount[0];
    final amount = invoiceAmount?.btc ?? 1.0;

    logger.i("Funding $address with $amount");
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet =
        const String.fromEnvironment("REGTEST_FAUCET", defaultValue: "http://34.32.62.120:8080");

    final data = {
      'jsonrpc': '1.0',
      'method': 'sendtoaddress',
      'params': [address, "$amount"],
    };
    final encodedData = json.encode(data);

    final response = await http.post(
      Uri.parse('$faucet/bitcoin'),
      headers: <String, String>{'Content-Type': 'text/plain'},
      body: encodedData,
    );

    if (response.statusCode != 200 || !response.body.contains('"error":null')) {
      throw Exception("Funding failed: Received ${response.statusCode} ${response.body}");
    } else {
      logger.i("Funding succeeded: ${response.body}");
    }

    {
      final data = {
        'jsonrpc': '1.0',
        'method': 'generatetoaddress',
        // a random address
        'params': [7, "bcrt1qlzarjr7s5fs983q7hcfuhnensrp9cs0n8gkacz"],
      };

      final encodedData = json.encode(data);

      final response = await http.post(
        Uri.parse('$faucet/bitcoin'),
        headers: <String, String>{'Content-Type': 'text/plain'},
        body: encodedData,
      );

      if (response.statusCode != 200 || !response.body.contains('"error":null')) {
        throw Exception("Mining blocks failed: received ${response.statusCode} ${response.body}");
      } else {
        logger.i("Mining blocks succeeded: ${response.body}");
      }
    }
  }

// Pay the generated invoice with maker faucet
  Future<void> payInvoiceWithMakerFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet = const String.fromEnvironment("REGTEST_MAKER_FAUCET",
        defaultValue: "http://34.32.62.120:80/maker/faucet");

    final response = await http.post(
      Uri.parse('$faucet/$invoice'),
    );

    logger.i("Response ${response.body}${response.statusCode}");

    if (response.statusCode != 200) {
      throw Exception("Payment failed: Received ${response.statusCode}. ${response.body}");
    } else {
      logger.i("Paying invoice succeeded: ${response.body}");
    }
  }
}
