package com.aitrack.server.config;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.MediaType;
import org.springframework.http.converter.ByteArrayHttpMessageConverter;
import org.springframework.http.converter.HttpMessageConverter;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

import java.util.List;

@Configuration
public class WebConfig implements WebMvcConfigurer {

    /**
     * Allows controllers to receive raw body as byte[] for JSON endpoints.
     * This preserves the exact bytes for HMAC signature verification before deserialization.
     */
    @Override
    public void configureMessageConverters(List<HttpMessageConverter<?>> converters) {
        ByteArrayHttpMessageConverter converter = new ByteArrayHttpMessageConverter();
        converter.setSupportedMediaTypes(List.of(
            MediaType.APPLICATION_JSON,
            MediaType.APPLICATION_OCTET_STREAM,
            MediaType.ALL
        ));
        converters.add(0, converter);
    }
}
